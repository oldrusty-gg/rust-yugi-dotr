//! Online Play as a scene of the connected game (a port addition, opened
//! from the title's "Online Play" entry): `online_menu::OnlineMenu` runs
//! once per 25 Hz update and `online_screen` draws it into a 512×512 frame
//! (the deck editor's present shader, as Custom Duel does). The server is
//! `online.json` in the game data folder (beside `memory-card/`, or
//! `ROSES_ONLINE_CONFIG`). The player brings a deck of MEMORY CARD slot 1's
//! save (else the game's card block, else the starter deck), a computer
//! opponent's deck or one of their custom decks (`online_decks`; chosen
//! before each match, the choice kept in `online-deck.json`; the custom
//! decks in `custom-decks.json`, both beside the session file, or
//! `ROSES_CUSTOM_DECKS`). EDIT CARDS opens the game's deck editor on the
//! full card pool (`deck_editor_render` in builder mode) and comes back.
//!
//! When the lobby pairs this player the game joins the match with the
//! lobby's ticket on a background thread ("Waiting for both duelists"),
//! then plays it as the existing online duel (`DuelPlay::online` on the
//! duel board with its 2-player result screens) and comes back here with
//! the server's result. The lobby connection is read during the duel too.
//!
//! Keys: arrows move, X / Enter confirm, Z / Escape back, D Triangle (a
//! deck's cards, a card's detail), A Square (COPY TO MY DECKS), 1 / W L2 /
//! R2 (the deck view's sort); on a text field the keyboard types (Enter
//! moves on, Backspace deletes).
//! Test aids: `ROSES_ONLINE_KEYS` (a script of keys, one per update:
//! `down`, `x`, `z`, `wait`, `"text"`, any of them `*N` times; and steps
//! that wait or act: `until:HEADING` (wait for that screen, `_` = space),
//! `player` (wait until another player is listed), `select:LABEL` (move
//! the cursor to that row), `shot:NAME` (write the frame to
//! `ROSES_ONLINE_SHOTS/NAME.png`), `roomout` / `roomin` (write the open
//! room's code to / type it from the file `ROSES_ONLINE_ROOM_FILE`; an
//! N-player room's too), `seated:N` (wait until the N-player room has N
//! players), `ok:LABEL` (Cross on that row, checked at the press), `exit`), `ROSES_ONLINE_CAPTURE=<new.png>`
//! written when the script is used up (or at update
//! `ROSES_ONLINE_CAPTURE_AT`, or once the screen's heading is
//! `ROSES_ONLINE_CAPTURE_HEADING`, e.g. RESULT after a duel), then the
//! game exits. MODES (a port addition, `duel_multi`): with
//! `--capture-game-scene online`, `ROSES_MULTI_CAPTURE_ROUND=N` lets the
//! N-player duel play on and captures it at rest once it reaches round N.

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{NoFrustumCulling, RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    input::{
        keyboard::{Key as LogicalKey, KeyboardInput},
        ButtonState,
    },
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use roses_game::duel_online::{ClientConn, Connector, OnlineLink};
use roses_game::online_menu::{DeckChoice, Key, MenuRequest, OnlineMenu};
use roses_game::online_screen::{OnlineScreen, FRAME};
use roses_game::{duel_play, memory_card_store as store};
use roses_core::card_block::CardBlock;
use roses_net::friends::{self, MatchReady, OnlineConfig, WatchReady};
use roses_net::{ClientMessage, Join, RulesTables, ServerMessage};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    Title,
    Duel,
    /// The deck builder on custom deck n.
    Build(usize),
}

#[derive(Clone, Debug)]
enum Scripted {
    Key(Key),
    /// A quoted string: replaces what the focused field holds (the log-in
    /// form keeps the last name, e.g. the one of a saved session the server
    /// refused, and typing after it made a wrong name).
    Text(String),
    Wait,
    /// Wait until the screen's heading is this.
    Until(String),
    /// Wait until another player is listed in the lobby.
    Player,
    /// Move the cursor to the row with this label.
    Select(String),
    /// Write the frame to `ROSES_ONLINE_SHOTS/<name>.png`.
    Shot(String),
    /// Write the open room's code to `ROSES_ONLINE_ROOM_FILE`.
    RoomOut,
    /// Type the room code from `ROSES_ONLINE_ROOM_FILE` (waits for it).
    RoomIn,
    /// MODES: wait until the N-player room has this many players.
    Seated(usize),
    /// Cross on the row with this label, checked at the press (a paced
    /// `select:`, then `x`, can land on another row when one is listed
    /// above it during the wait, e.g. a player coming online); off the row
    /// it steps toward it first.
    Ok(String),
    Exit,
}

/// The Online Play flow: lives from the title's entry until the menu
/// returns to the title (kept across the duel).
#[derive(Resource)]
pub struct OnlineState {
    /// In a mutex only so the resource is `Sync` (always reached through
    /// `&mut`): the lobby connection and the joining thread's receiver are
    /// `Send` but not `Sync`.
    menu_cell: std::sync::Mutex<OnlineMenu>,
    screen: Option<OnlineScreen>,
    image: Option<Handle<Image>>,
    pending: Vec<Key>,
    cards: roses_core::duel::Catalogue,
    tables: RulesTables,
    /// The player's own block (the save's), for the duel's result screens.
    block: Option<CardBlock>,
    joining: std::sync::Mutex<Option<mpsc::Receiver<Result<OnlineLink, String>>>>,
    link: std::sync::Mutex<Option<OnlineLink>>,
    in_duel: bool,
    /// The game's card block before the duel (the result screens write
    /// into the blocks they are given; online play changes no save).
    saved_block: Option<Option<CardBlock>>,
    request: Option<Request>,
    updates: u64,
    script: VecDeque<Scripted>,
    /// The update the script's current step started waiting at.
    step_since: u64,
    reported: std::collections::BTreeSet<String>,
    /// The custom deck the deck builder is open on.
    building: Option<usize>,
    /// Video aid: `ROSES_BOT_PACE=human` waits before each scripted key as
    /// a person would (`video_bot::Pacer`, seeded by `ROSES_BOT_SEED`).
    pacer: Option<roses_game::video_bot::Pacer>,
}

impl OnlineState {
    fn menu(&mut self) -> &mut OnlineMenu {
        self.menu_cell.get_mut().unwrap_or_else(|e| e.into_inner())
    }
    fn joining(&mut self) -> &mut Option<mpsc::Receiver<Result<OnlineLink, String>>> {
        self.joining.get_mut().unwrap_or_else(|e| e.into_inner())
    }
    fn link(&mut self) -> &mut Option<OnlineLink> {
        self.link.get_mut().unwrap_or_else(|e| e.into_inner())
    }
}

/// The server file and the session file: `ROSES_ONLINE_CONFIG` (session
/// beside it), or the game data folder's `online.json` (else the default
/// the package ships beside the executables) and
/// `roses-data/online-session.json`, which the launcher's Profile page
/// writes too.
fn config_paths(root: &std::path::Path) -> (PathBuf, PathBuf) {
    match std::env::var_os("ROSES_ONLINE_CONFIG") {
        Some(p) => {
            let p = PathBuf::from(p);
            let session = p.with_file_name(roses_net::friends::SESSION_FILE);
            (p, session)
        }
        None => {
            let data = root.parent().unwrap_or(root);
            (roses_net::friends::find_config(data), data.join(roses_net::friends::SESSION_FILE))
        }
    }
}

fn parse_script(text: &str) -> VecDeque<Scripted> {
    let mut out = VecDeque::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('"') {
            let end = r.find('"').unwrap_or(r.len());
            out.push_back(Scripted::Text(r[..end].to_string()));
            rest = r.get(end + 1..).unwrap_or("").trim_start_matches([',', ' ']);
            continue;
        }
        let (word, r) = rest.split_once([',', ' ']).unwrap_or((rest, ""));
        rest = r.trim_start_matches([',', ' ']);
        let (word, count) = match word.rsplit_once('*') {
            Some((w, n)) if n.parse::<usize>().is_ok() => (w, n.parse::<usize>().unwrap_or(1)),
            _ => (word, 1),
        };
        let arg = |prefix: &str| word.get(prefix.len()..).unwrap_or("").replace('_', " ");
        let lower = word.to_ascii_lowercase();
        let step = match lower.as_str() {
            "up" => Scripted::Key(Key::Up),
            "down" => Scripted::Key(Key::Down),
            "left" => Scripted::Key(Key::Left),
            "right" => Scripted::Key(Key::Right),
            "x" | "cross" | "enter" => Scripted::Key(Key::Cross),
            "z" | "circle" | "back" => Scripted::Key(Key::Circle),
            "d" | "triangle" => Scripted::Key(Key::Triangle),
            "a" | "square" => Scripted::Key(Key::Square),
            "l2" => Scripted::Key(Key::L2),
            "r2" => Scripted::Key(Key::R2),
            "bs" | "backspace" => Scripted::Key(Key::Backspace),
            "player" => Scripted::Player,
            "roomout" => Scripted::RoomOut,
            "roomin" => Scripted::RoomIn,
            "exit" => Scripted::Exit,
            w if w.starts_with("until:") => Scripted::Until(arg("until:")),
            w if w.starts_with("select:") => Scripted::Select(arg("select:")),
            w if w.starts_with("ok:") => Scripted::Ok(arg("ok:")),
            w if w.starts_with("shot:") => Scripted::Shot(word["shot:".len()..].to_string()),
            w if w.starts_with("seated:") => Scripted::Seated(w["seated:".len()..].parse().unwrap_or(1)),
            _ => Scripted::Wait,
        };
        out.extend(std::iter::repeat_n(step, count.max(1)));
    }
    out
}

/// One step of the `ROSES_ONLINE_KEYS` script; false = not done yet (the
/// step waits for its condition and is tried again next update).
fn run_step(st: &mut OnlineState, step: &Scripted, world: &mut World) -> bool {
    use roses_game::online_menu::Row;
    let view = st.menu().view();
    match step {
        Scripted::Key(k) => st.pending.push(*k),
        Scripted::Text(text) => {
            if let Some(Row::Field(_, value)) = view.rows.get(view.cursor).filter(|_| view.typing) {
                st.pending.extend(std::iter::repeat_n(Key::Backspace, value.chars().count()));
            }
            st.pending.extend(text.chars().map(Key::Char));
        }
        Scripted::Wait => {}
        Scripted::Until(h) => return view.heading.eq_ignore_ascii_case(h),
        Scripted::Player => {
            return view.heading == "LOBBY"
                && view.rows.iter().any(|r| matches!(r, Row::Player(_, roses_net::lobby::PlayerState::Idle)));
        }
        Scripted::Select(label) => {
            let Some(i) = view.rows.iter().position(|r| row_is(r, label)) else { return false };
            let key = if i < view.cursor { Key::Up } else { Key::Down };
            st.pending.extend(std::iter::repeat_n(key, i.abs_diff(view.cursor)));
        }
        Scripted::Shot(name) => {
            let dir = std::env::var_os("ROSES_ONLINE_SHOTS").map_or_else(|| PathBuf::from("."), PathBuf::from);
            let path = dir.join(format!("{name}.png"));
            if let Some(screen) = st.screen.as_ref() {
                match super::deck_editor_render::write_png(&path, &screen.rgba()) {
                    Ok(()) => eprintln!("Online script: frame written to {}", path.display()),
                    Err(e) => eprintln!("Online script: shot {name} failed: {e}"),
                }
            }
        }
        Scripted::RoomOut => {
            let room = |l: &roses_net::friends::LobbyClient| l.room.clone().or_else(|| l.mode_room.as_ref().map(|r| r.code.clone()));
            let Some(code) = st.menu().lobby.as_ref().and_then(room) else { return false };
            if let Some(file) = std::env::var_os("ROSES_ONLINE_ROOM_FILE") {
                let _ = std::fs::write(file, &code);
            }
            eprintln!("Online script: room code {code}");
        }
        Scripted::RoomIn => {
            let Some(code) = std::env::var_os("ROSES_ONLINE_ROOM_FILE").and_then(|f| std::fs::read_to_string(f).ok()) else {
                return false;
            };
            let code = code.trim();
            if code.is_empty() {
                return false;
            }
            st.pending.extend(code.chars().map(Key::Char));
        }
        Scripted::Ok(label) => {
            let Some(i) = view.rows.iter().position(|r| row_is(r, label)) else { return false };
            if i == view.cursor {
                st.pending.push(Key::Cross);
                return true;
            }
            st.pending.push(if i < view.cursor { Key::Up } else { Key::Down });
            return false;
        }
        Scripted::Seated(n) => {
            let players = st.menu().mode_room().map_or(0, |r| r.seats.iter().filter(|s| s.player.is_some()).count());
            return players >= *n;
        }
        Scripted::Exit => {
            eprintln!("Online script: exit");
            world.write_message(AppExit::Success);
        }
    }
    true
}

/// Whether a menu row has this label (`select:` / `ok:`).
fn row_is(r: &roses_game::online_menu::Row, label: &str) -> bool {
    use roses_game::online_menu::Row;
    match r {
        Row::Button(t) | Row::Field(t, _) | Row::Player(t, _) | Row::Choice(t, _) => t.eq_ignore_ascii_case(label),
        Row::Deck { name, .. } => name.eq_ignore_ascii_case(label),
        // A seat by its letter or who sits there ("select:E").
        Row::Seat { seat, name, .. } => seat.eq_ignore_ascii_case(label) || name.eq_ignore_ascii_case(label),
        // A live duel by either player's name ("select:Joey").
        Row::Live { title, .. } => title.eq_ignore_ascii_case(label) || title.split(" vs ").any(|n| n.eq_ignore_ascii_case(label)),
    }
}

/// Video aid: the wait kind of a scripted menu key at a human pace.
fn key_moment(k: Key) -> roses_game::video_bot::Moment {
    use roses_game::video_bot::Moment;
    match k {
        Key::Up | Key::Down | Key::Left | Key::Right => Moment::MenuMove,
        Key::Cross => Moment::MenuConfirm,
        Key::Char(_) | Key::Backspace => Moment::Type,
        _ => Moment::Menu,
    }
}

/// Video aid: at a human pace a `select:` step moves the cursor one key at
/// a time and a typed text becomes its single characters, so each waits.
fn paced_expand(st: &mut OnlineState) {
    use roses_game::online_menu::Row;
    let Some(step) = st.script.front().cloned() else { return };
    let view = st.menu().view();
    let keys: Vec<Key> = match &step {
        Scripted::Select(label) => {
            let Some(i) = view.rows.iter().position(|r| row_is(r, label)) else { return };
            if i == view.cursor {
                return;
            }
            // One step at a time, the label looked up again before each:
            // rows can move under the cursor (a player coming online is
            // listed above the buttons).
            let key = if i < view.cursor { Key::Up } else { Key::Down };
            st.script.push_front(Scripted::Key(key));
            return;
        }
        Scripted::Text(text) => {
            let mut keys = Vec::new();
            if let Some(Row::Field(_, value)) = view.rows.get(view.cursor).filter(|_| view.typing) {
                keys.extend(std::iter::repeat_n(Key::Backspace, value.chars().count()));
            }
            keys.extend(text.chars().map(Key::Char));
            keys
        }
        _ => return,
    };
    st.script.pop_front();
    for k in keys.into_iter().rev() {
        st.script.push_front(Scripted::Key(k));
    }
}

/// The deck and block this player brings: MEMORY CARD slot 1's save, else
/// the game's card block, else the starter deck of the duel import.
fn choose_deck(flow: &super::GameFlow, data: &duel_play::DuelData) -> (Result<DeckChoice, String>, Option<CardBlock>) {
    let card = store::VirtualCard::slot1(&flow.import.root);
    if let Ok(bytes) = std::fs::read(card.data_path()) {
        match store::restore(&bytes) {
            Ok(r) => {
                let deck = roses_net::deck::from_card_block(&r.block, None);
                let n = roses_core::deck_editor::current_deck(&r.block) + 1;
                return (deck.map(|deck| DeckChoice { deck, source: format!("your save, deck {n}") }), Some(r.block));
            }
            Err(e) => eprintln!("Online: MEMORY CARD slot 1 cannot be read ({e}); using another deck"),
        }
    }
    if let Some(block) = &flow.player_block {
        if let Ok(deck) = roses_net::deck::from_card_block(block, None) {
            return (Ok(DeckChoice { deck, source: "this game's deck".into() }), Some(block.clone()));
        }
    }
    let side = &data.first_duel.sides[0];
    let deck = roses_net::deck::from_import_side(&roses_net::data::ImportSide { leader: side.leader, cards: side.cards.clone() });
    (Ok(DeckChoice { deck, source: "the starter deck (no save yet)".into() }), None)
}

/// Every deck the player can bring: the save's (`save`), the import's
/// starter deck, the computer opponents' of the user's executable (named
/// from their MSG_E.DAT) and the custom decks file.
fn deck_library(
    data: &duel_play::DuelData,
    save: Option<CardBlock>,
    session: &std::path::Path,
) -> roses_game::online_decks::DeckLibrary {
    let side = &data.first_duel.sides[0];
    let starter = roses_net::deck::from_import_side(&roses_net::data::ImportSide { leader: side.leader, cards: side.cards.clone() });
    let opponents = roses_net::deck::opponent_decks(&data.executable, Some(&data.messages)).unwrap_or_else(|e| {
        eprintln!("Online: the computer opponents' decks are unavailable: {e}");
        Vec::new()
    });
    let first = roses_core::duel_end::message::RANK_NAME_FIRST;
    let ids: Vec<u16> = (first..=first + 12).collect();
    let rank_names = roses_net::deck::message_texts(&data.messages, &ids).unwrap_or_default();
    let custom = std::env::var_os("ROSES_CUSTOM_DECKS")
        .map(PathBuf::from)
        .unwrap_or_else(|| session.with_file_name(roses_net::custom_decks::CUSTOM_DECKS_FILE));
    let mut library = roses_game::online_decks::DeckLibrary::new(
        save,
        Some(starter),
        opponents,
        &custom,
        data.names.clone(),
        rank_names,
    );
    if let Some(cards) = roses_core::deck_editor::StaticCards::from_executable(&data.executable, 0) {
        library = library.with_cards(cards);
    }
    // The game's boards for the map choice (none: only own maps and RANDOM).
    match roses_assets::parse_pal_rules(&data.executable) {
        Ok(rules) => library = library.with_maps(rules.maps.into_iter().map(|m| m.cells).collect()),
        Err(e) => eprintln!("Online: the game's boards are unavailable for the map choice: {e}"),
    }
    eprintln!(
        "Online: decks: {} computer opponents, {} custom ({}){}",
        library.opponents.len(),
        library.custom.decks.len(),
        custom.display(),
        library.custom_error.as_ref().map(|e| format!("; the custom decks file is not usable: {e}")).unwrap_or_default()
    );
    library
}

/// Whether the deck builder is open on a custom deck.
pub fn building(world: &World) -> bool {
    world.get_resource::<OnlineState>().is_some_and(|s| s.building.is_some())
}

/// The deck builder's inputs when Online Play asked for it: the game's
/// deck editor on the custom deck's full-pool block. `ROSES_BUILDER_REPLAY`
/// (pad words in hex, comma separated) scripts its pad (test aid).
pub fn builder_inputs(world: &mut World) -> Option<Result<super::deck_editor_render::EditorInputs, String>> {
    let index = world.get_resource::<OnlineState>()?.building?;
    Some((|| {
        let deck = {
            let mut st = world.resource_mut::<OnlineState>();
            st.menu().custom(index).cloned().ok_or("the custom deck is gone")?
        };
        let flow = world.resource::<super::GameFlow>();
        let data = duel_play::load_with_end(&flow.import.duel, flow.import.duel_end.as_deref())?;
        let cards = roses_core::deck_editor::StaticCards::from_executable(&data.executable, 0)
            .ok_or("the executable is too short for the card records")?;
        let block = roses_net::custom_decks::editor_block(&deck, &cards);
        eprintln!("Online: the deck builder opens on {:?} ({} cards, leader {})", deck.name, deck.cards.len(), deck.leader);
        let replay: Vec<u16> = std::env::var("ROSES_BUILDER_REPLAY")
            .ok()
            .map(|v| v.split(',').filter_map(|w| u16::from_str_radix(w.trim().trim_start_matches("0x"), 16).ok()).collect())
            .unwrap_or_default();
        // Video aid: at a human pace each scripted press waits first.
        let replay = {
            use roses_game::video_bot as vb;
            let c = vb::BotConfig::from_env();
            vb::Pacer::new(c.pace, c.seed ^ 0xB1D).pace_words(&replay).into()
        };
        Ok(super::deck_editor_render::EditorInputs {
            side: 0,
            block,
            duel_counter: 0,
            rng: flow.rng,
            executable: data.executable,
            messages: data.messages,
            texsys: data.texsys,
            picmini: data.picmini,
            names: data.names,
            replay,
            blink: flow.scroll_word,
            blink_p1: flow.scroll_p1,
            d63: flow.d63,
            builder: true,
        })
    })())
}

/// The deck builder closed: the custom deck takes its cards and leader.
pub fn builder_closed(world: &mut World, result: super::deck_editor_render::EditorResult) {
    let Some(mut st) = world.get_resource_mut::<OnlineState>() else { return };
    let Some(index) = st.building.take() else { return };
    let Some(before) = st.menu().custom(index).cloned() else { return };
    let deck = roses_net::custom_decks::from_editor_block(&result.block, &before);
    eprintln!(
        "Online: the deck builder closed after {} updates: {:?} {} cards, leader {} ({})",
        result.frames,
        deck.name,
        deck.cards.len(),
        deck.leader,
        deck.problem().unwrap_or_else(|| "ready".into())
    );
    st.menu().deck_edited(index, deck);
}

/// Whether the Online Play flow is active (the duel returns to it).
pub fn active(world: &World) -> bool {
    world.contains_resource::<OnlineState>()
}

/// Enter (or come back from the duel to) the Online Play screens.
pub fn enter_scene(world: &mut World) -> Result<(), String> {
    if !world.contains_resource::<OnlineState>() {
        let (root, duel_dir) = {
            let flow = world.resource::<super::GameFlow>();
            (flow.import.root.clone(), flow.import.duel.clone())
        };
        let data = duel_play::load(&duel_dir)?;
        let tables = RulesTables::from_executable(&data.executable)?;
        let compat = roses_net::Compat::current(roses_net::data_fingerprint(&data.catalogue, &tables));
        let (deck, block) = choose_deck(world.resource::<super::GameFlow>(), &data);
        let (path, session) = config_paths(&root);
        let library = deck_library(&data, block.clone(), &session);
        let choice_path = session.with_file_name(roses_game::online_decks::CHOICE_FILE);
        let menu = OnlineMenu::with_session(&path, &session, deck, compat).with_library(library, &choice_path);
        eprintln!("Game: Online Play: server file {} ({})", path.display(), if menu.config.is_ok() { "found" } else { "missing" });
        let mut screen = OnlineScreen::new(&data.executable, &data.messages, &data.texsys, &data.picmini)?;
        // PICTURE.MRG (the duel-end import) for the deck view's detail pane.
        let end_dir = world.resource::<super::GameFlow>().import.duel_end.clone();
        if let Some(dir) = end_dir {
            match roses_game::duel_end_screens::load_duel_end_import(&dir)
                .and_then(|files| screen.set_pictures(files.pictures, &data.executable))
            {
                Ok(()) => {}
                Err(e) => eprintln!("Online: no card pictures for the detail pane ({e}): it shows NOW LOADING"),
            }
        }
        world.insert_resource(OnlineState {
            menu_cell: std::sync::Mutex::new(menu),
            screen: Some(screen),
            image: None,
            pending: Vec::new(),
            cards: data.catalogue,
            tables,
            block,
            joining: std::sync::Mutex::new(None),
            link: std::sync::Mutex::new(None),
            in_duel: false,
            saved_block: None,
            request: None,
            updates: 0,
            script: std::env::var("ROSES_ONLINE_KEYS").map(|s| parse_script(&s)).unwrap_or_default(),
            step_since: 0,
            reported: Default::default(),
            building: None,
            pacer: {
                use roses_game::video_bot as vb;
                let c = vb::BotConfig::from_env();
                (c.pace == vb::Pace::Human).then(|| vb::Pacer::new(c.pace, c.seed ^ 0x3E_7A))
            },
        });
    }
    let restore = {
        let mut st = world.resource_mut::<OnlineState>();
        if std::mem::take(&mut st.in_duel) {
            eprintln!("Game: Online Play: back from the duel to the lobby");
            st.menu().returned();
            st.saved_block.take()
        } else {
            None
        }
    };
    if let Some(block) = restore {
        world.resource_mut::<super::GameFlow>().player_block = block;
    }
    if let Some(mut sound) = world.get_resource_mut::<super::duel_board_render::DuelSound>() {
        sound.cue(roses_game::duel_audio::AudioCue::Music { track: 3, keep: true });
    }
    world.run_system_cached(setup_scene).map_err(|e| format!("online setup: {e}"))?;
    // The title's caption would stay otherwise.
    super::set_title(world, "Roses - Online Play");
    Ok(())
}

pub fn add_systems(app: &mut App) {
    let on = resource_exists::<OnlineState>.and_then(in_scene);
    app.add_systems(
        RunFixedMainLoop,
        keys.run_if(on.clone()).in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(FixedUpdate, tick.run_if(on))
    .add_systems(FixedUpdate, pump_during_duel.run_if(resource_exists::<OnlineState>.and_then(not(in_scene))));
}

fn in_scene(flow: Option<Res<super::GameFlow>>) -> bool {
    flow.is_some_and(|f| f.scene == super::GameScene::Online && f.next.is_none())
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<super::deck_editor_render::EditorMaterial>>,
    mut st: ResMut<OnlineState>,
) {
    let st = &mut *st;
    let view = st.menu().view();
    let Some(screen) = st.screen.as_mut() else { return };
    screen.draw(&view, false);
    let mut image = Image::new(
        Extent3d { width: FRAME as u32, height: FRAME as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        screen.rgba(),
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
    st.image = Some(image.clone());
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

/// Keyboard (and the gamepad, which reaches scenes as keys) to menu keys;
/// on a text field the keyboard types.
fn keys(mut events: MessageReader<KeyboardInput>, mut st: ResMut<OnlineState>, pads: Option<ResMut<super::pad_input::Pads>>) {
    if let Some(mut p) = pads {
        p.pad1_extra();
    }
    for e in events.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        let typing = st.menu().wants_text();
        let key = match (&e.key_code, &e.logical_key) {
            (KeyCode::ArrowUp, _) => Some(Key::Up),
            (KeyCode::ArrowDown | KeyCode::Tab, _) => Some(Key::Down),
            (KeyCode::ArrowLeft, _) if !typing => Some(Key::Left),
            (KeyCode::ArrowRight, _) if !typing => Some(Key::Right),
            (KeyCode::Enter | KeyCode::NumpadEnter, _) => Some(Key::Cross),
            (KeyCode::Escape, _) => Some(Key::Circle),
            (KeyCode::Backspace, _) if typing => Some(Key::Backspace),
            (_, LogicalKey::Character(s)) if typing => {
                for c in s.chars() {
                    st.pending.push(Key::Char(c));
                }
                None
            }
            (_, LogicalKey::Space) if typing => Some(Key::Char(' ')),
            (KeyCode::KeyX, _) if !e.repeat => Some(Key::Cross),
            // The pad's Triangle, Square, L2 and R2 keys (the deck view).
            (KeyCode::KeyD, _) if !e.repeat => Some(Key::Triangle),
            (KeyCode::KeyA, _) if !e.repeat => Some(Key::Square),
            (KeyCode::Digit1, _) if !e.repeat => Some(Key::L2),
            (KeyCode::KeyW, _) if !e.repeat => Some(Key::R2),
            (KeyCode::KeyZ | KeyCode::Backspace, _) if !e.repeat => Some(Key::Circle),
            _ => None,
        };
        if let Some(k) = key {
            st.pending.push(k);
        }
    }
}

/// Join the match the lobby arranged on a background thread.
fn start_joining(st: &mut OnlineState, ready: MatchReady) {
    let Ok(config) = st.menu().config.clone() else { return };
    let Ok(deck) = st.menu().deck.as_ref().map(|d| d.deck.clone()) else {
        st.menu().match_failed("no deck to play with");
        return;
    };
    let (cards, tables) = (st.cards.clone(), st.tables.clone());
    let (tx, rx) = mpsc::channel();
    eprintln!("Online: the lobby paired this player with {} (seat {}); joining {}", ready.opponent, ready.seat, ready.match_id);
    std::thread::spawn(move || {
        let config: OnlineConfig = config;
        let connector: Connector =
            Box::new(move || friends::connect::<ClientMessage, ServerMessage>(&config).map(|c| Box::new(c) as ClientConn));
        let link = OnlineLink::connect(
            connector,
            &cards,
            &tables,
            Join::Ticket { ticket: ready.ticket },
            Some(deck),
            Duration::from_secs(180),
            |line| eprintln!("Online: {line}"),
        );
        let _ = tx.send(link);
    });
    *st.joining() = Some(rx);
}

/// Watch the duel the lobby let this player watch, joining on a
/// background thread (a spectator: no deck, no seat).
fn start_watching(st: &mut OnlineState, w: WatchReady) {
    let Ok(config) = st.menu().config.clone() else { return };
    let (cards, tables) = (st.cards.clone(), st.tables.clone());
    let (tx, rx) = mpsc::channel();
    eprintln!("Online: watching {} vs {} ({})", w.players[0], w.players[1], w.match_id);
    std::thread::spawn(move || {
        let config: OnlineConfig = config;
        let connector: Connector =
            Box::new(move || friends::connect::<ClientMessage, ServerMessage>(&config).map(|c| Box::new(c) as ClientConn));
        let _ = tx.send(OnlineLink::watch(connector, &cards, &tables, w.ticket, Duration::from_secs(60)));
    });
    *st.joining() = Some(rx);
}

/// One 25 Hz update of the screens.
fn tick(world: &mut World) {
    let Some(mut st) = world.remove_resource::<OnlineState>() else { return };
    st.updates += 1;
    if st.pacer.is_some() {
        paced_expand(&mut st);
    }
    let held = match (st.script.front(), st.pacer.as_mut()) {
        (Some(Scripted::Key(k)), Some(pacer)) => !pacer.ready(key_moment(*k)),
        (Some(Scripted::Ok(_)), Some(pacer)) => !pacer.ready(roses_game::video_bot::Moment::MenuConfirm),
        _ => false,
    };
    if let Some(step) = st.script.front().cloned().filter(|_| !held) {
        if run_step(&mut st, &step, world) {
            if !matches!(step, Scripted::Key(_) | Scripted::Wait | Scripted::Text(_)) {
                eprintln!("Online script: {step:?} done at update {}", st.updates);
            }
            st.script.pop_front();
            st.step_since = st.updates;
        } else if (st.updates - st.step_since) % 250 == 0 {
            // Every 10 s: what the step is waiting on (a stalled run says why).
            let view = st.menu().view();
            eprintln!(
                "Online script: {step:?} still waiting at update {}; the screen is {} (note: {})",
                st.updates,
                view.heading,
                view.note.as_deref().unwrap_or("none")
            );
        }
    }
    for k in std::mem::take(&mut st.pending) {
        st.menu().press(k);
    }
    st.menu().update();
    match st.menu().request.take() {
        Some(MenuRequest::Title) => st.request = Some(Request::Title),
        Some(MenuRequest::Play(ready)) => start_joining(&mut st, ready),
        Some(MenuRequest::EditDeck(i)) => st.request = Some(Request::Build(i)),
        Some(MenuRequest::Watch(w)) => start_watching(&mut st, w),
        Some(MenuRequest::PlayMulti(ready)) => {
            let names: Vec<String> = ready.seats.iter().map(|s| s.name.clone()).collect();
            eprintln!("Online: an N-player match ({:?}): {names:?}; this player sits at seat {}", ready.settings.mode, ready.seat);
            let opponent = format!("{} players", ready.seats.len());
            let paired = MatchReady {
                match_id: ready.match_id,
                seat: ready.seat,
                ticket: ready.ticket,
                opponent,
                options: Default::default(),
                map: ready.settings.map,
                opponent_title: None,
            };
            start_joining(&mut st, paired);
        }
        None => {}
    }
    let joined = st.joining().as_ref().map(|rx| rx.try_recv());
    if let Some(joined) = joined {
        match joined {
            Ok(Ok(link)) if link.spectator => {
                eprintln!("Online: watching {} (the view of nobody)", link.state.match_id.clone().unwrap_or_default());
                *st.joining() = None;
                *st.link() = Some(link);
                st.request = Some(Request::Duel);
            }
            Ok(Ok(link)) => {
                eprintln!("Online: the match began; this game plays seat {} ({}P)", link.seat, link.seat + 1);
                *st.joining() = None;
                *st.link() = Some(link);
                st.request = Some(Request::Duel);
            }
            Ok(Err(e)) => {
                eprintln!("Online: could not join the match: {e}");
                *st.joining() = None;
                st.menu().match_failed(&e);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                *st.joining() = None;
                st.menu().match_failed("joining the match stopped");
            }
        }
    }
    if let Some(mut sound) = world.get_resource_mut::<super::duel_board_render::DuelSound>() {
        for id in std::mem::take(&mut st.menu().sfx) {
            sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id, skip: false });
        }
    } else {
        st.menu().sfx.clear();
    }
    let view = st.menu().view();
    let blink = st.updates / 25 % 2 == 0;
    if let Some(screen) = st.screen.as_mut() {
        for n in screen.draw(&view, blink) {
            if st.reported.insert(n.clone()) {
                eprintln!("Online Play: not drawn: {n}");
            }
        }
        if let Some(handle) = st.image.clone() {
            let rgba = screen.rgba();
            if let Some(mut image) = world.resource_mut::<Assets<Image>>().get_mut(&handle) {
                image.data = Some(rgba);
            }
        }
    }
    if st.script.is_empty() && st.updates > 1 && std::env::var_os("ROSES_ONLINE_KEYS").is_some() {
        if let Some(mut flow) = world.get_resource_mut::<super::GameFlow>() {
            flow.online_script_done = true;
        }
    }
    let capture_now = match (
        std::env::var("ROSES_ONLINE_CAPTURE_AT").ok().and_then(|v| v.parse::<u64>().ok()),
        std::env::var("ROSES_ONLINE_CAPTURE_HEADING").ok(),
    ) {
        (Some(n), _) => st.updates == n,
        (None, Some(heading)) => st.script.is_empty() && view.heading.eq_ignore_ascii_case(&heading),
        (None, None) => st.script.is_empty() && std::env::var_os("ROSES_ONLINE_KEYS").is_some() && st.updates > 1,
    };
    if capture_now {
        if let (Some(path), Some(screen)) = (std::env::var_os("ROSES_ONLINE_CAPTURE"), st.screen.as_ref()) {
            let path = PathBuf::from(path);
            match super::deck_editor_render::write_png(&path, &screen.rgba()) {
                Ok(()) => eprintln!("Online Play: frame written to {} (update {})", path.display(), st.updates),
                Err(e) => eprintln!("Online Play: capture failed: {e}"),
            }
            world.write_message(AppExit::Success);
        }
    }
    world.insert_resource(st);
}

/// Keep the lobby connection read while the duel runs.
fn pump_during_duel(mut st: ResMut<OnlineState>) {
    st.menu().update();
    st.menu().sfx.clear();
}

/// The online duel stopped before its result screens (an error, or no
/// duel-end import): the menu says so when the screens come back.
pub fn duel_stopped(world: &mut World, why: &str) {
    if let Some(mut st) = world.get_resource_mut::<OnlineState>() {
        st.menu().note = Some(why.chars().take(roses_game::online_menu::LINE_MAX).collect());
    }
}

/// Called by the game's watch while the screens are up: the scene they
/// asked for.
pub fn watch(world: &mut World) {
    let Some(request) = world.get_resource_mut::<OnlineState>().and_then(|mut s| s.request.take()) else { return };
    match request {
        Request::Title => {
            eprintln!("Game: Online Play: back to the title");
            world.remove_resource::<OnlineState>();
            let mut flow = world.resource_mut::<super::GameFlow>();
            flow.story_stage = Some("title".into());
            flow.next = Some(super::GameScene::Opening);
        }
        Request::Duel => {
            let mut flow = world.resource_mut::<super::GameFlow>();
            flow.won = None;
            flow.request = None;
            flow.next = Some(super::GameScene::Duel);
        }
        Request::Build(index) => {
            eprintln!("Game: Online Play: the deck builder on custom deck {index}");
            world.resource_mut::<OnlineState>().building = Some(index);
            world.resource_mut::<super::GameFlow>().next = Some(super::GameScene::DeckEditor);
        }
    }
}

/// The online duel scene when Online Play has a match to play (`None`
/// otherwise): the duel board with the online duel and the 2-player result
/// screens.
pub fn duel_scene(world: &mut World) -> Option<Result<(), String>> {
    let link = world.get_resource_mut::<OnlineState>()?.link().take()?;
    Some((|| {
        let flow = world.resource::<super::GameFlow>();
        let data = duel_play::load_with_end(&flow.import.duel, flow.import.duel_end.as_deref())?;
        let board = flow.import.duel_board_manifest.clone();
        let models = flow.import.duel_models_manifest.clone();
        let (arena, audio, duel_end) = (flow.import.duel_arena.clone(), flow.import.duel_audio.clone(), flow.import.duel_end.clone());
        let saved = flow.player_block.clone();
        // `--end-replay` scripts the result screens' pad here too (test aid).
        let end_replay = flow.end_replay.clone();
        let mut rng = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        let names = world.resource_mut::<OnlineState>().menu().seat_names();
        // MODES (a port addition): an N-player match shows every seat's name.
        let multi_names = world.resource_mut::<OnlineState>().menu().multi_names();
        let multi = multi_names.is_some();
        let mut play = match multi_names {
            Some(all) => duel_play::DuelPlay::online_multi(data, link, all)?,
            None => duel_play::DuelPlay::online(data, link)?,
        };
        // The HUD's plates show the players' names (the computer as its
        // deck's character).
        play.seat_names = if multi { None } else { names };
        let end = match duel_end {
            // An N-player match ends with its placing list.
            Some(_) if multi => None,
            Some(dir) => {
                use roses_game::duel_end_screens as screens;
                let files = screens::load_duel_end_import(&dir)?;
                let thresholds = roses_assets::parse_pal_rank_thresholds(&play.data.executable).map_err(|e| e.to_string())?;
                let mut st = world.resource_mut::<OnlineState>();
                // Both sides' blocks for the result screens: this player's
                // own, and a copy for the other seat (its block is never
                // sent; the 2-player result shows no ranks or prizes).
                let deck = st.menu().deck.as_ref().map(|d| d.deck.clone()).ok();
                let block = st.block.clone().unwrap_or_else(|| {
                    let (cards, leader, rank) = deck.map_or((Vec::new(), 0, 1), |d| (d.cards, d.leader, d.leader_rank));
                    screens::new_game_block(1, &cards, leader, u16::from(rank), &thresholds, &mut rng)
                });
                Some(super::duel_board_render::EndInputs {
                    blocks: [block.clone(), block],
                    rng,
                    map: play.data.first_duel.board_map as u8,
                    duel_end: files,
                    replay: end_replay,
                })
            }
            None => None,
        };
        let scene = super::duel_board_render::prepare_online_scene(&board, play, end, Some(models.as_path()))?;
        let scene = match &arena {
            Some(a) => scene.with_arena(a)?,
            None => scene,
        };
        let scene = match &audio {
            Some(a) => scene.with_audio(a)?,
            None => scene,
        };
        {
            let mut st = world.resource_mut::<OnlineState>();
            st.in_duel = true;
            st.saved_block = Some(saved);
        }
        scene.enter(world)
    })())
}
