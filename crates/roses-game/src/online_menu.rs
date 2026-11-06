//! The Online Play screens (a port addition opened from the title's
//! "Online Play" entry), Bevy-free: the server from `online.json`, log in
//! or sign up (with an invite code only when the server asks for one;
//! a session saved by the launcher's Profile page or an earlier run signs
//! in by itself as soon as the screen opens), the players online, challenges, private
//! rooms by code, the server's computer opponent, then the match (played
//! by the existing online duel, `duel_online`) and back here with its
//! result. With a deck library (`online_decks`) the player chooses the deck
//! to bring before each challenge, answer, room or computer match (the
//! last choice remembered), and DECKS manages their own custom decks (new,
//! copy, name, leader rank, duplicate, delete; EDIT CARDS opens the game's
//! deck editor on every card, `MenuRequest::EditDeck`). Triangle on any
//! deck (or VIEW CARDS) opens the deck view (`Screen::DeckView`,
//! `online_decks::DeckPreview`): its leader and rank and its cards, sorted
//! with L2 / R2 (or Left / Right), Triangle for the card's detail pane, and
//! Square to COPY TO MY DECKS ("NAME (copy)", opened in the deck builder;
//! COPY A DECK does the same). With a server that takes match choices
//! (`LobbyClient::choices`), a challenge, a room and PLAY COMPUTER then ask
//! for the board (`Screen::MapSelect`: RANDOM, the game's VS and campaign
//! boards, the save's own maps, with Custom Duel's terrain preview), and
//! PLAY COMPUTER first for the computer's deck (`Screen::ComputerSelect`:
//! RANDOM, the computer opponents' decks, the player's own); the board a
//! challenger chose is shown to the one challenged before they answer.
//! With a server that offers spectating (`LobbyClient::spectate`), WATCH A
//! DUEL lists the live duels that may be watched (`Screen::Watch`: the
//! players, the board, the round, how many watch; refreshed every few
//! seconds) and Cross watches one (`MenuRequest::Watch`: the game joins as
//! a spectator, sees no hand and no face-down card, and Start brings it
//! back here); ALLOW WATCHING (kept in `online-watching.json` beside the
//! session, on by default) lets others watch this player's duels or not.
//! `online_screen` draws [`OnlineMenu::view`] with the game's own
//! menu pieces; `online_render` (the game binary) runs it once per update.

use roses_net::friends::{self, LobbyClient, LobbyConn, MatchReady, MultiMatchReady, OnlineConfig, SavedSession, WatchReady};
use roses_net::lobby::{
    check_password, check_username, normalize_code, ComputerDeck, LiveMatch, LobbyEvent, LobbyRequest, MapChoice, MatchResult,
    PlayerState, RoomInfo, RoomMode, RoomSettings, ROOM_CODE_LEN, ROOM_LP_CHOICES, ROOM_TURN_CHOICES,
};
use crate::online_decks::{self, DeckEntry, DeckLibrary, DeckPreview, DeckRef};
use roses_net::custom_decks::{self, CustomDeck};
use roses_net::lobby::AuthProblem;
use roses_net::{Compat, DeckSubmission};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// SYSTEM0 sounds of the original's menus (custom_duel.rs).
pub const SFX_CURSOR: u8 = 3;
pub const SFX_CONFIRM: u8 = 4;
pub const SFX_CANCEL: u8 = 5;
pub const SFX_BUZZER: u8 = 6;
/// The longest line the screen draws (the text canvas holds 42 cells).
pub const LINE_MAX: usize = 40;
/// A lobby connection is kept alive with a ping this often.
const PING_EVERY: Duration = Duration::from_secs(30);
/// The WATCH list is asked for again this often while it is shown.
pub const WATCH_REFRESH: Duration = Duration::from_secs(3);
/// The "Allow spectators" setting, beside the session file.
pub const WATCHING_FILE: &str = "online-watching.json";

/// The saved "Allow spectators" setting (on when there is none).
pub fn load_allow_watching(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("allow_spectators").and_then(serde_json::Value::as_bool))
        .unwrap_or(true)
}

pub fn save_allow_watching(path: &Path, allow: bool) -> std::io::Result<()> {
    std::fs::write(path, serde_json::json!({ "allow_spectators": allow }).to_string())
}

/// One input: a pad press or, while a text field is chosen, a typed key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    /// Confirm (Cross / Enter).
    Cross,
    /// Back (Circle / Escape).
    Circle,
    /// Detail (Triangle / D): a deck's cards, a card's detail pane.
    Triangle,
    /// Square (A): COPY TO MY DECKS in the deck view.
    Square,
    /// L2 / R2 (1 / W): the deck view's sort.
    L2,
    R2,
    Char(char),
    Backspace,
}

/// The deck this player brings (their own save's current deck).
#[derive(Clone, Debug)]
pub struct DeckChoice {
    pub deck: DeckSubmission,
    /// Where it came from, for the screen ("MEMORY CARD slot 1 deck 1").
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Password,
    Invite,
    RoomCode,
    /// The name of the custom deck being looked at.
    DeckName,
}

/// What the deck chosen in [`Screen::DeckSelect`] is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Challenge(String),
    Accept { id: u64, from: String },
    CreateRoom,
    JoinRoom,
    Computer,
    /// Copy it as a new custom deck.
    Copy,
    /// MODES (a port addition): open an N-player room.
    CreateModeRoom,
    /// MODES: join an N-player room by its code.
    JoinModeRoom,
    /// MODES: the board of the room being set up (no deck chosen).
    ModeBoard,
}

/// A setting of the MODES room setup (changed with Left / Right).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeRow {
    Mode,
    CardsLeave,
    StartLp,
    SharedLp,
    TurnTimer,
    Spectators,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    /// No usable `online.json`.
    NoServer,
    Server,
    Connecting,
    Account,
    Form { signup: bool },
    Signing,
    Lobby,
    Challenged { id: u64, from: String },
    Sent { id: u64, to: String },
    RoomOpen,
    RoomCode,
    /// The lobby paired this player; the game joins the match.
    Starting,
    /// Back from a duel, until the server's result arrives.
    AwaitResult { since: Instant },
    Result { result: MatchResult },
    /// Choose the deck to bring (or to copy).
    DeckSelect { then: Pending },
    /// The custom decks.
    Decks,
    /// One custom deck.
    DeckMenu { index: usize },
    DeleteDeck { index: usize },
    /// A deck looked at card by card (`OnlineMenu::preview`); Circle goes
    /// back to `back` with the cursor at `back_cursor`.
    DeckView { back: Box<Screen>, back_cursor: usize },
    /// Choose the board of a challenge, a room or a computer match.
    MapSelect { then: Pending },
    /// Choose the computer's deck (PLAY COMPUTER).
    ComputerSelect,
    /// The live duels that may be watched (WATCH A DUEL).
    Watch,
    /// The server let this player watch; the game joins as a spectator.
    WatchStarting,
    /// MODES (a port addition, an invented mode): the N-player room's
    /// settings before it is opened (or, `editing`, the host changing an
    /// open room's).
    ModeSetup { editing: bool },
    /// MODES: the N-player room this player is in (its seats).
    ModeRoom,
    /// MODES: type the code of an N-player room.
    ModeRoomCode,
}

/// What the menu asks the game for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuRequest {
    Title,
    Play(MatchReady),
    /// Open the deck builder on custom deck `index` (the game answers with
    /// [`OnlineMenu::deck_edited`]).
    EditDeck(usize),
    /// Watch this duel as a spectator.
    Watch(WatchReady),
    /// MODES: play the N-player match the lobby arranged.
    PlayMulti(MultiMatchReady),
}

/// A row of the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Button(String),
    /// A text field: label, shown value (passwords masked), focused.
    Field(String, String),
    /// Another player online.
    Player(String, PlayerState),
    /// A deck: its name, its list's tag (SAVE / MINE / CPU), its card
    /// count or what it lacks, and whether it can be played.
    Deck { name: String, tag: String, detail: String, ready: bool },
    /// A setting changed with Left / Right: label and value.
    Choice(String, String),
    /// A live duel to watch: "A VS B", its round, how many watch.
    Live { title: String, round: u8, watching: u16 },
    /// MODES: a seat of the N-player room: its letter (S, N, E, W), who
    /// sits there (a name, COMPUTER or OPEN), its team in 2v2 and whether
    /// it is this player's.
    Seat { seat: String, name: String, team: Option<String>, you: bool },
}

/// A deck's row.
fn deck_row(e: &DeckEntry) -> Row {
    Row::Deck {
        name: e.name.clone(),
        tag: e.source.tag().into(),
        detail: match &e.deck {
            Ok(d) => format!("{}", d.cards.len()),
            Err(_) => format!("{}/40", e.count),
        },
        ready: e.deck.is_ok(),
    }
}

/// The deck view's content (drawn as the deck editor's list).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeckView {
    pub name: String,
    /// SAVE / MINE / CPU.
    pub tag: String,
    /// 999 = none.
    pub leader: u16,
    pub leader_name: String,
    pub rank: u8,
    pub rank_name: String,
    /// Each card once with its copies, in the sort's order.
    pub groups: Vec<(u16, u8)>,
    /// Row 0 is the leader.
    pub cursor: usize,
    pub first: usize,
    pub sort_key: u8,
    pub sort_label: String,
    /// The detail pane's page while it is open.
    pub detail: Option<u8>,
    pub count: usize,
    /// What the deck lacks before it can be played.
    pub problem: Option<String>,
    /// What Cross does here, if anything.
    pub cross: Option<String>,
    /// Square copies it to the custom decks.
    pub can_copy: bool,
}

/// A board's terrain preview (Custom Duel's): the cells, none for RANDOM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapPreview {
    pub cells: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    /// The panel's heading (gold font: letters, digits, spaces).
    pub heading: String,
    pub info: Vec<String>,
    pub rows: Vec<Row>,
    pub cursor: usize,
    /// The last thing to tell the player.
    pub note: Option<String>,
    /// Typing goes into the chosen field.
    pub typing: bool,
    /// The deck view, on that screen.
    pub deck: Option<DeckView>,
    /// The board's terrain preview (the map choice, a challenge).
    pub map: Option<MapPreview>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Connect,
    Back,
    Title,
    LogIn,
    SignUp,
    Submit,
    Field(Field),
    Challenge(usize),
    CreateRoom,
    JoinRoom,
    Computer,
    LogOut,
    Accept,
    Decline,
    CancelChallenge,
    CloseRoom,
    Join,
    Ok,
    Decks,
    /// Choose entry `i` of the deck list.
    PickDeck(usize),
    NewDeck,
    CopyDeck,
    OpenDeck(usize),
    EditDeck,
    /// VIEW CARDS: the deck view of the custom deck looked at.
    ViewDeck,
    /// Map choice row `i`.
    PickMap(usize),
    /// The computer's deck row `i` (0 = RANDOM).
    PickComputer(usize),
    LeaderRank,
    Duplicate,
    Delete,
    ConfirmDelete,
    /// WATCH A DUEL: the list of live duels.
    Watch,
    /// Watch live duel `i` of the list.
    WatchMatch(usize),
    /// ALLOW WATCHING: on / off.
    AllowWatching,
    /// MODES: the room setup.
    Modes,
    /// A room setting (Cross steps it; Left / Right too).
    ModeSetting(ModeRow),
    /// The room's board (the map choice).
    ModeBoard,
    ModeCreate,
    ModeJoin,
    /// JOIN on the code screen.
    ModeJoinCode,
    /// The host's settings applied to the open room.
    ModeApply,
    /// The host opens the settings of the open room.
    ModeEdit,
    /// Seat `i` of the room: sit there.
    Seat(u8),
    ModeStart,
    ModeLeave,
}

pub struct OnlineMenu {
    pub config: Result<OnlineConfig, String>,
    pub config_path: PathBuf,
    session_path: PathBuf,
    pub deck: Result<DeckChoice, String>,
    compat: Compat,
    pub screen: Screen,
    pub cursor: usize,
    pub lobby: Option<LobbyClient>,
    connecting: Option<mpsc::Receiver<Result<LobbyConn, String>>>,
    pub name: String,
    pub password: String,
    pub invite: String,
    pub room_code: String,
    pub note: Option<String>,
    pub sfx: Vec<u8>,
    pub request: Option<MenuRequest>,
    last_ping: Instant,
    /// Whether the last sign-in used the saved session.
    resuming: bool,
    /// The form last submitted was a sign-up.
    signup: bool,
    /// Every deck the player can bring (none: the deck given at start is
    /// the only one, with no deck screens).
    pub library: Option<DeckLibrary>,
    /// The remembered choice and where it is kept.
    pub choice: Option<DeckRef>,
    choice_path: Option<PathBuf>,
    /// The name typed for the custom deck being looked at.
    pub deck_name: String,
    /// The deck of [`Screen::DeckView`].
    pub preview: Option<DeckPreview>,
    /// The computer's deck chosen for PLAY COMPUTER.
    pub computer_choice: Option<ComputerDeck>,
    /// The board of the challenge being answered.
    pub challenge_map: Option<MapChoice>,
    /// The map row chosen last (the next map choice starts there).
    last_map: usize,
    /// The last match's opponent as shown (the result screen names them).
    pub last_opponent: Option<String>,
    /// "Allow spectators" (ALLOW WATCHING) and where it is kept.
    pub allow_watching: bool,
    watching_path: PathBuf,
    /// When the WATCH list was last asked for.
    last_list: Instant,
    /// The duel being watched (back from it: the WATCH list).
    pub watching: Option<WatchReady>,
    /// MODES: the settings of the room being set up.
    pub mode_settings: RoomSettings,
    /// MODES: the N-player match being played (its seats' names).
    pub multi_active: Option<MultiMatchReady>,
    /// MODES: the seats' names of the N-player match being watched.
    pub watch_seats: Option<Vec<String>>,
}

/// "< VALUE >" for a setting row.
fn choice(v: &str) -> String {
    format!("< {v} >")
}

/// The mode's name as the screens show it.
pub fn mode_name(mode: RoomMode) -> &'static str {
    match mode {
        RoomMode::FreeForAll => "FREE-FOR-ALL",
        RoomMode::Teams => "2 VS 2",
    }
}

/// One line saying what a room's settings are.
pub fn settings_line(s: &RoomSettings) -> String {
    let clock = if s.turn_seconds == 0 { "no clock".to_string() } else { format!("{} s", s.turn_seconds) };
    let lp = if s.mode == RoomMode::Teams && s.shared_lp { "team LP" } else { "LP" };
    format!(
        "{}, {} {lp}, {clock}, cards {}",
        mode_name(s.mode),
        s.start_lp,
        if s.cards_leave { "leave" } else { "stay" }
    )
}

/// The next value of a list of choices (`delta` ±1, wrapping).
fn step_in<T: PartialEq + Copy>(list: &[T], value: T, delta: i32) -> T {
    let n = list.len() as i32;
    let at = list.iter().position(|&v| v == value).map_or(0, |i| i as i32);
    list[(at + delta).rem_euclid(n) as usize]
}

/// A room setting stepped by `delta`.
pub fn step_setting(s: &mut RoomSettings, row: ModeRow, delta: i32) {
    match row {
        ModeRow::Mode => {
            s.mode = if s.mode == RoomMode::FreeForAll { RoomMode::Teams } else { RoomMode::FreeForAll };
            if s.mode == RoomMode::FreeForAll {
                s.shared_lp = false;
            }
        }
        ModeRow::CardsLeave => s.cards_leave = !s.cards_leave,
        ModeRow::StartLp => s.start_lp = step_in(&ROOM_LP_CHOICES, s.start_lp, delta),
        ModeRow::SharedLp => s.shared_lp = !s.shared_lp && s.mode == RoomMode::Teams,
        ModeRow::TurnTimer => s.turn_seconds = step_in(&ROOM_TURN_CHOICES, s.turn_seconds, delta),
        ModeRow::Spectators => s.spectators = !s.spectators,
    }
}

/// The seat letters of an N-player room (S, N, E, W).
pub const SEAT_NAMES: [&str; 4] = ["S", "N", "E", "W"];

fn cut(s: &str) -> String {
    s.chars().take(LINE_MAX).collect()
}

/// The server could not be reached: the operating system's own sentence is
/// longer than the note's line, so it goes to the log.
fn unreachable_note(detail: &str) -> String {
    eprintln!("Online: cannot reach the server: {detail}");
    "cannot reach the server; try again later".into()
}

/// A sign-in problem for the note box, short enough for its line.
fn auth_note(problem: AuthProblem) -> String {
    match problem {
        AuthProblem::BadUsername => "3-12 letters or digits, a letter first".into(),
        AuthProblem::SignUpLimit => "too many new accounts; try later".into(),
        AuthProblem::EmailNotVerified => "verify your email in the launcher first".into(),
        p => cut(&p.to_string()),
    }
}

/// Names by seat as `", "`-joined lines of [`LINE_MAX`] letters at most.
pub fn name_lines(names: &[String]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for name in names {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 2 + name.chars().count() < LINE_MAX => {
                l.push_str(", ");
                l.push_str(name);
            }
            _ => lines.push(cut(name)),
        }
    }
    lines
}

impl OnlineMenu {
    /// `config_path`: `online.json`; the session is kept next to it.
    pub fn new(config_path: &Path, deck: Result<DeckChoice, String>, compat: Compat) -> Self {
        Self::with_session(config_path, &config_path.with_file_name(friends::SESSION_FILE), deck, compat)
    }

    /// `session_path`: where the signed-in session is kept (the game data
    /// folder's `online-session.json`, shared with the launcher). With a
    /// session saved for this server the menu connects and signs in at
    /// once.
    pub fn with_session(config_path: &Path, session_path: &Path, deck: Result<DeckChoice, String>, compat: Compat) -> Self {
        let config = if config_path.is_file() {
            OnlineConfig::load(config_path)
        } else {
            Err(format!("{} does not exist", config_path.display()))
        };
        let screen = if config.is_ok() { Screen::Server } else { Screen::NoServer };
        let mut menu = Self {
            config,
            config_path: config_path.to_path_buf(),
            session_path: session_path.to_path_buf(),
            deck,
            compat,
            screen,
            cursor: 0,
            lobby: None,
            connecting: None,
            name: String::new(),
            password: String::new(),
            invite: String::new(),
            room_code: String::new(),
            note: None,
            sfx: Vec::new(),
            request: None,
            last_ping: Instant::now(),
            resuming: false,
            signup: false,
            library: None,
            choice: None,
            choice_path: None,
            deck_name: String::new(),
            preview: None,
            computer_choice: None,
            challenge_map: None,
            last_map: 0,
            last_opponent: None,
            allow_watching: load_allow_watching(&session_path.with_file_name(WATCHING_FILE)),
            watching_path: session_path.with_file_name(WATCHING_FILE),
            last_list: Instant::now(),
            watching: None,
            mode_settings: RoomSettings::default(),
            multi_active: None,
            watch_seats: None,
        };
        if menu.saved_session().is_some() {
            menu.act(Action::Connect);
            menu.sfx.clear();
        }
        menu
    }

    /// Give the menu every deck the player can bring; the remembered choice
    /// (`choice_path`, else the save's current deck) becomes the deck when
    /// it can be played.
    pub fn with_library(mut self, library: DeckLibrary, choice_path: &Path) -> Self {
        let choice = online_decks::load_choice(choice_path).unwrap_or(DeckRef::SaveCurrent);
        self.library = Some(library);
        self.choice_path = Some(choice_path.to_path_buf());
        self.use_choice(&choice);
        self
    }

    /// Make `choice` the deck when it exists and can be played.
    fn use_choice(&mut self, choice: &DeckRef) {
        let Some(entry) = self.library.as_ref().and_then(|l| l.entry(choice)) else { return };
        if let Ok(deck) = entry.deck {
            self.deck = Ok(DeckChoice { deck, source: entry.name });
            self.choice = Some(choice.clone());
        }
    }

    fn deck_entries(&self) -> Vec<DeckEntry> {
        self.library.as_ref().map(|l| l.entries()).unwrap_or_default()
    }

    fn save_decks(&mut self) {
        if let Some(Err(e)) = self.library.as_ref().map(|l| l.save_custom()) {
            self.sfx.push(SFX_BUZZER);
            self.note = Some(cut(&e));
        }
    }

    /// The deck builder closed on custom deck `index`: keep its cards and
    /// leader (the name and rank stay as set here).
    pub fn deck_edited(&mut self, index: usize, deck: CustomDeck) {
        let Some(lib) = self.library.as_mut() else { return };
        let Some(d) = lib.custom.decks.get_mut(index) else { return };
        d.cards = deck.cards;
        d.leader = deck.leader;
        self.save_decks();
        // The deck in hand follows its edits.
        if let Some(choice) = self.choice.clone() {
            self.use_choice(&choice);
        }
        self.open_deck(index);
    }

    /// The custom deck `index`.
    pub fn custom(&self, index: usize) -> Option<&CustomDeck> {
        self.library.as_ref().and_then(|l| l.custom.decks.get(index))
    }

    fn open_deck(&mut self, index: usize) {
        self.deck_name = self.custom(index).map(|d| d.name.clone()).unwrap_or_default();
        self.go(Screen::DeckMenu { index });
    }

    /// Commit a typed deck name (on leaving the field or the screen).
    fn commit_name(&mut self) {
        let Screen::DeckMenu { index } = self.screen else { return };
        let Some(lib) = self.library.as_mut() else { return };
        let Some(old) = lib.custom.decks.get(index).map(|d| d.name.clone()) else { return };
        if self.deck_name == old {
            return;
        }
        let was_chosen = self.choice == Some(DeckRef::Custom { name: old.clone() });
        let new = lib.custom.rename(index, &self.deck_name).unwrap_or(old);
        self.deck_name = new.clone();
        self.save_decks();
        if was_chosen {
            let choice = DeckRef::Custom { name: new };
            self.remember(&choice);
            self.use_choice(&choice);
        }
    }

    fn remember(&mut self, choice: &DeckRef) {
        self.choice = Some(choice.clone());
        if let Some(path) = &self.choice_path {
            if let Err(e) = online_decks::save_choice(path, choice) {
                self.note = Some(cut(&format!("the deck choice was not saved: {e}")));
            }
        }
    }

    /// Before a match: the deck list (cursor on the last choice), or the
    /// action at once without a library.
    fn choose_deck_then(&mut self, then: Pending) {
        if self.library.is_none() {
            self.after_deck(then);
            return;
        }
        self.sfx.push(SFX_CONFIRM);
        let entries = self.deck_entries();
        let at = match (&then, &self.choice) {
            (Pending::Copy, _) | (_, None) => 0,
            (_, Some(c)) => entries.iter().position(|e| &e.reference == c).unwrap_or(0),
        };
        self.note = None;
        self.go(Screen::DeckSelect { then });
        self.cursor = at;
    }

    /// Whether the server takes map and computer-deck choices (and the
    /// game has the lists).
    pub fn choices(&self) -> bool {
        self.library.is_some() && self.lobby.as_ref().is_some_and(|l| l.choices)
    }

    /// The map rows (the library's choices).
    fn map_entries(&self) -> Vec<MapChoice> {
        self.library.as_ref().map(|l| l.map_entries()).unwrap_or_default()
    }

    /// The computer's deck rows after RANDOM: every deck that can be
    /// played, the computer opponents' first.
    fn computer_entries(&self) -> Vec<DeckEntry> {
        let mut e: Vec<DeckEntry> = self.deck_entries().into_iter().filter(|e| e.deck.is_ok()).collect();
        e.sort_by_key(|e| match e.source {
            online_decks::Source::Opponent => 0,
            online_decks::Source::Custom => 1,
            online_decks::Source::Save => 2,
        });
        e
    }

    fn choose_map(&mut self, then: Pending) {
        self.sfx.push(SFX_CONFIRM);
        self.note = None;
        self.go(Screen::MapSelect { then });
        self.cursor = self.last_map.min(self.map_entries().len().saturating_sub(1));
    }

    /// The board is chosen (none: the server's default): send the request.
    fn send_with_map(&mut self, then: Pending, map: Option<MapChoice>) {
        self.sfx.push(SFX_CONFIRM);
        match then {
            Pending::Challenge(to) => self.lobby_send(LobbyRequest::Challenge { to, map }),
            Pending::CreateRoom => self.lobby_send(LobbyRequest::CreateRoom { map }),
            Pending::Computer => {
                let computer = self.computer_choice.clone();
                self.lobby_send(LobbyRequest::PlayComputer { map, computer });
            }
            Pending::ModeBoard => {
                self.mode_settings.map = map;
                let editing = self.mode_room().is_some();
                self.go(Screen::ModeSetup { editing });
                // The cursor back on the BOARD row.
                self.cursor = self.rows().iter().position(|(_, a)| *a == Action::ModeBoard).unwrap_or(0);
                return;
            }
            _ => {}
        }
        self.go(Screen::Lobby);
    }

    /// The deck is chosen: do what it was chosen for.
    fn after_deck(&mut self, then: Pending) {
        match then {
            Pending::Challenge(_) | Pending::CreateRoom if self.choices() => self.choose_map(then),
            Pending::Computer if self.choices() => {
                self.sfx.push(SFX_CONFIRM);
                self.note = None;
                self.go(Screen::ComputerSelect);
            }
            Pending::Challenge(to) => {
                self.sfx.push(SFX_CONFIRM);
                self.lobby_send(LobbyRequest::Challenge { to, map: None });
                self.go(Screen::Lobby);
            }
            Pending::Accept { id, .. } => {
                self.sfx.push(SFX_CONFIRM);
                self.lobby_send(LobbyRequest::Answer { id, accept: true });
                self.go(Screen::Lobby);
            }
            Pending::CreateRoom => {
                self.sfx.push(SFX_CONFIRM);
                self.lobby_send(LobbyRequest::CreateRoom { map: None });
                self.go(Screen::Lobby);
            }
            Pending::JoinRoom => {
                self.sfx.push(SFX_CONFIRM);
                self.room_code.clear();
                self.go(Screen::RoomCode);
            }
            Pending::Computer => {
                self.sfx.push(SFX_CONFIRM);
                self.lobby_send(LobbyRequest::PlayComputer { map: None, computer: None });
                self.go(Screen::Lobby);
            }
            Pending::Copy => {}
            Pending::CreateModeRoom => {
                self.sfx.push(SFX_CONFIRM);
                let settings = self.mode_settings.clone();
                self.lobby_send(LobbyRequest::CreateModeRoom { settings });
                self.note = None;
                self.go(Screen::ModeRoom);
            }
            Pending::JoinModeRoom => {
                self.sfx.push(SFX_CONFIRM);
                self.room_code.clear();
                self.go(Screen::ModeRoomCode);
            }
            Pending::ModeBoard => {}
        }
    }

    fn saved_session(&self) -> Option<SavedSession> {
        SavedSession::load(&self.session_path, &self.config.as_ref().ok()?.server)
    }

    fn save_session(&self, username: &str, session: &str) {
        let Ok(config) = &self.config else { return };
        let s = SavedSession { server: config.server.clone(), username: username.into(), session: session.into() };
        let _ = s.save(&self.session_path);
    }

    fn forget_session(&self) {
        SavedSession::forget(&self.session_path);
    }

    /// Whether sign-up needs an invite code (the server says; until it
    /// has, yes).
    pub fn invite_required(&self) -> bool {
        self.lobby.as_ref().is_none_or(|l| l.invite_required)
    }

    fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.cursor = 0;
    }

    /// Whether the server lets players watch duels.
    pub fn spectate(&self) -> bool {
        self.lobby.as_ref().is_some_and(|l| l.spectate)
    }

    /// The live duels the server listed last.
    pub fn live(&self) -> &[LiveMatch] {
        self.lobby.as_ref().and_then(|l| l.live.as_deref()).unwrap_or(&[])
    }

    fn ask_for_list(&mut self) {
        self.last_list = Instant::now();
        self.lobby_send(LobbyRequest::ListMatches);
    }

    /// A live duel's title: "YUGI VS KAIBA".
    pub fn live_title(m: &LiveMatch) -> String {
        // MODES (a port addition): every seat of an N-player match.
        if m.seats.len() > 2 {
            return m.seats.join(", ");
        }
        format!("{} vs {}", m.players[0], m.players[1])
    }

    /// A live duel's row in the WATCH list: its title, or for an N-player
    /// match the first player and how many more (four names do not fit).
    pub fn live_row_title(m: &LiveMatch) -> String {
        match m.seats.split_first() {
            // The message font has no '+'.
            Some((first, rest)) if m.seats.len() > 2 => format!("{first} &{}", rest.len()),
            _ => Self::live_title(m),
        }
    }

    /// The rows of this screen and what each does.
    fn rows(&self) -> Vec<(Row, Action)> {
        let b = |t: &str, a: Action| (Row::Button(t.into()), a);
        let field = |f: Field, label: &str, value: &str| (Row::Field(label.into(), value.into()), Action::Field(f));
        match &self.screen {
            Screen::NoServer => vec![b("BACK", Action::Title)],
            Screen::Server if self.library.is_some() => {
                vec![b("CONNECT", Action::Connect), b("DECKS", Action::Decks), b("BACK", Action::Title)]
            }
            Screen::Server => vec![b("CONNECT", Action::Connect), b("BACK", Action::Title)],
            Screen::Connecting | Screen::Signing => vec![b("CANCEL", Action::Back)],
            Screen::Account => vec![b("LOG IN", Action::LogIn), b("SIGN UP", Action::SignUp), b("BACK", Action::Back)],
            Screen::Form { signup } => {
                let mut rows = vec![
                    field(Field::Name, "NAME", &self.name),
                    field(Field::Password, "PASSWORD", &"#".repeat(self.password.chars().count())),
                ];
                if *signup && self.invite_required() {
                    rows.push(field(Field::Invite, "INVITE CODE", &self.invite));
                }
                rows.push(b(if *signup { "SIGN UP" } else { "LOG IN" }, Action::Submit));
                rows.push(b("BACK", Action::Back));
                rows
            }
            Screen::Lobby => {
                let me = self.lobby.as_ref().and_then(|l| l.user.clone()).unwrap_or_default();
                let mut rows: Vec<(Row, Action)> = self
                    .lobby
                    .iter()
                    .flat_map(|l| l.players.iter())
                    .filter(|p| p.name != me)
                    .enumerate()
                    .map(|(i, p)| (Row::Player(p.name.clone(), p.state), Action::Challenge(i)))
                    .collect();
                rows.push(b("CREATE ROOM", Action::CreateRoom));
                rows.push(b("JOIN ROOM", Action::JoinRoom));
                if self.lobby.as_ref().and_then(|l| l.server.as_ref()).is_some_and(|s| s.1) {
                    rows.push(b("PLAY COMPUTER", Action::Computer));
                }
                if self.spectate() {
                    rows.push(b("WATCH A DUEL", Action::Watch));
                }
                if self.modes() {
                    rows.push(b("MODES", Action::Modes));
                }
                if self.library.is_some() {
                    rows.push(b("DECKS", Action::Decks));
                }
                if self.spectate() {
                    let value = if self.allow_watching { "< YES >" } else { "< NO >" };
                    rows.push((Row::Choice("ALLOW WATCHING".into(), value.into()), Action::AllowWatching));
                }
                rows.push(b("LOG OUT", Action::LogOut));
                rows.push(b("BACK TO TITLE", Action::Title));
                rows
            }
            Screen::Challenged { .. } => vec![b("ACCEPT", Action::Accept), b("DECLINE", Action::Decline)],
            Screen::Sent { .. } => vec![b("CANCEL", Action::CancelChallenge)],
            Screen::RoomOpen => vec![b("CLOSE ROOM", Action::CloseRoom)],
            Screen::RoomCode => vec![field(Field::RoomCode, "ROOM CODE", &self.room_code), b("JOIN", Action::Join), b("BACK", Action::Back)],
            Screen::Starting | Screen::AwaitResult { .. } => vec![],
            Screen::Result { .. } => vec![b("OK", Action::Ok)],
            Screen::DeckSelect { .. } => self.deck_entries().iter().enumerate().map(|(i, e)| (deck_row(e), Action::PickDeck(i))).collect(),
            Screen::Decks => {
                let mut rows = vec![b("NEW DECK", Action::NewDeck), b("COPY A DECK", Action::CopyDeck)];
                if let Some(lib) = &self.library {
                    for i in 0..lib.custom.decks.len() {
                        if let Some(e) = lib.entry(&DeckRef::Custom { name: lib.custom.decks[i].name.clone() }) {
                            rows.push((deck_row(&e), Action::OpenDeck(i)));
                        }
                    }
                }
                rows.push(b("BACK", Action::Back));
                rows
            }
            Screen::DeckMenu { index } => {
                let rank = self.custom(*index).map_or(1, |d| d.leader_rank);
                let rank_name = self.library.as_ref().map(|l| l.rank_name(rank)).unwrap_or_default();
                vec![
                    b("EDIT CARDS", Action::EditDeck),
                    b("VIEW CARDS", Action::ViewDeck),
                    field(Field::DeckName, "NAME", &self.deck_name),
                    (Row::Choice("LEADER RANK".into(), format!("< {rank_name} >")), Action::LeaderRank),
                    b("DUPLICATE", Action::Duplicate),
                    b("DELETE", Action::Delete),
                    b("BACK", Action::Back),
                ]
            }
            Screen::DeleteDeck { .. } => vec![b("DELETE", Action::ConfirmDelete), b("KEEP", Action::Back)],
            Screen::DeckView { .. } => vec![],
            Screen::MapSelect { .. } => self
                .map_entries()
                .iter()
                .enumerate()
                .map(|(i, m)| (Row::Button(online_decks::map_label(m)), Action::PickMap(i)))
                .collect(),
            Screen::ComputerSelect => {
                let mut rows = vec![(Row::Button("RANDOM".into()), Action::PickComputer(0))];
                rows.extend(self.computer_entries().iter().enumerate().map(|(i, e)| (deck_row(e), Action::PickComputer(i + 1))));
                rows
            }
            Screen::Watch => {
                let mut rows: Vec<(Row, Action)> = self
                    .live()
                    .iter()
                    .enumerate()
                    .map(|(i, m)| (Row::Live { title: Self::live_row_title(m), round: m.round, watching: m.watching }, Action::WatchMatch(i)))
                    .collect();
                rows.push(b("BACK", Action::Back));
                rows
            }
            Screen::WatchStarting => vec![],
            Screen::ModeSetup { editing } => {
                let s = &self.mode_settings;
                let set = |label: &str, value: String, row: ModeRow| (Row::Choice(label.into(), choice(&value)), Action::ModeSetting(row));
                let mut rows = vec![
                    set("MODE", mode_name(s.mode).into(), ModeRow::Mode),
                    set("KNOCKED-OUT CARDS", if s.cards_leave { "LEAVE" } else { "STAY" }.into(), ModeRow::CardsLeave),
                    set("START LP", s.start_lp.to_string(), ModeRow::StartLp),
                ];
                if s.mode == RoomMode::Teams {
                    rows.push(set("TEAM LP", if s.shared_lp { "SHARED" } else { "SEPARATE" }.into(), ModeRow::SharedLp));
                }
                let clock = if s.turn_seconds == 0 { "NO CLOCK".to_string() } else { format!("{} S", s.turn_seconds) };
                rows.push(set("TURN TIMER", clock, ModeRow::TurnTimer));
                rows.push(set("SPECTATORS", if s.spectators { "ALLOW" } else { "NO" }.into(), ModeRow::Spectators));
                if self.choices() {
                    let board = s.map.as_ref().map_or("SERVER'S".to_string(), online_decks::map_label);
                    rows.push((Row::Choice("BOARD".into(), board), Action::ModeBoard));
                }
                if *editing {
                    rows.push(b("APPLY", Action::ModeApply));
                } else {
                    rows.push(b("CREATE ROOM", Action::ModeCreate));
                    rows.push(b("JOIN ROOM", Action::ModeJoin));
                }
                rows.push(b("BACK", Action::Back));
                rows
            }
            Screen::ModeRoom => {
                let Some(room) = self.mode_room() else {
                    return vec![b("LEAVE", Action::ModeLeave)];
                };
                let me = self.lobby.as_ref().and_then(|l| l.user.clone());
                let teams = room.settings.mode == RoomMode::Teams;
                let mut rows: Vec<(Row, Action)> = room
                    .seats
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let name = match (&s.player, s.computer) {
                            (Some(p), _) => p.clone(),
                            (None, true) => "COMPUTER".into(),
                            (None, false) => "OPEN".into(),
                        };
                        let team = teams.then(|| format!("TEAM {}", if s.team == 0 { "A" } else { "B" }));
                        let you = s.player.is_some() && s.player == me;
                        let seat = SEAT_NAMES.get(i).copied().unwrap_or("?").to_string();
                        (Row::Seat { seat, name, team, you }, Action::Seat(i as u8))
                    })
                    .collect();
                if self.is_host() {
                    rows.push(b("SETTINGS", Action::ModeEdit));
                    rows.push(b("START", Action::ModeStart));
                }
                rows.push(b("LEAVE", Action::ModeLeave));
                rows
            }
            Screen::ModeRoomCode => {
                vec![field(Field::RoomCode, "ROOM CODE", &self.room_code), b("JOIN", Action::ModeJoinCode), b("BACK", Action::Back)]
            }
        }
    }

    /// Whether the server offers the N-player modes (MODES).
    pub fn modes(&self) -> bool {
        self.lobby.as_ref().is_some_and(|l| l.modes)
    }

    /// The N-player room this player is in.
    pub fn mode_room(&self) -> Option<&RoomInfo> {
        self.lobby.as_ref()?.mode_room.as_ref()
    }

    /// This player hosts the N-player room.
    pub fn is_host(&self) -> bool {
        let me = self.lobby.as_ref().and_then(|l| l.user.clone());
        self.mode_room().is_some_and(|r| Some(&r.host) == me.as_ref())
    }

    /// Square on a seat (the host): the computer plays it, or it opens.
    fn toggle_computer(&mut self, seat: u8) {
        let Some(s) = self.mode_room().and_then(|r| r.seats.get(usize::from(seat))).cloned() else { return };
        if !self.is_host() {
            self.sfx.push(SFX_BUZZER);
            self.note = Some("only the room's host can do that".into());
            return;
        }
        if s.player.is_some() {
            self.sfx.push(SFX_BUZZER);
            self.note = Some("a player sits there".into());
            return;
        }
        self.sfx.push(SFX_CURSOR);
        self.lobby_send(LobbyRequest::ModeRoomComputer { seat, computer: !s.computer });
    }

    fn focused_field(&self) -> Option<Field> {
        match self.rows().get(self.cursor) {
            Some((_, Action::Field(f))) => Some(*f),
            _ => None,
        }
    }

    /// Whether typed characters go into a field now.
    pub fn wants_text(&self) -> bool {
        self.focused_field().is_some()
    }

    fn field_mut(&mut self, f: Field) -> &mut String {
        match f {
            Field::Name => &mut self.name,
            Field::Password => &mut self.password,
            Field::Invite => &mut self.invite,
            Field::RoomCode => &mut self.room_code,
            Field::DeckName => &mut self.deck_name,
        }
    }

    pub fn press(&mut self, key: Key) {
        if matches!(self.screen, Screen::DeckView { .. }) {
            self.press_view(key);
            return;
        }
        let rows = self.rows();
        let n = rows.len();
        if let (Some(f), Key::Char(c)) = (self.focused_field(), key) {
            let (max, ok) = match f {
                Field::Name => (roses_net::lobby::USERNAME_MAX, c.is_ascii_alphanumeric()),
                Field::Password => (roses_net::lobby::PASSWORD_MAX, c.is_ascii_graphic() || c == ' '),
                Field::Invite => (9, c.is_ascii_alphanumeric() || c == '-'),
                Field::RoomCode => (ROOM_CODE_LEN, c.is_ascii_alphanumeric()),
                Field::DeckName => (custom_decks::NAME_MAX, custom_decks::name_char_ok(c)),
            };
            let value = self.field_mut(f);
            if ok && value.chars().count() < max {
                value.push(if matches!(f, Field::Invite | Field::RoomCode) { c.to_ascii_uppercase() } else { c });
                self.sfx.push(SFX_CURSOR);
            } else {
                self.sfx.push(SFX_BUZZER);
            }
            return;
        }
        // A typed deck name is kept when the cursor leaves its row.
        if self.focused_field() == Some(Field::DeckName) && matches!(key, Key::Up | Key::Down | Key::Cross | Key::Circle) {
            self.commit_name();
        }
        // LEADER RANK: Left / Right choose it.
        if let (Some((_, Action::LeaderRank)), Key::Left | Key::Right) = (rows.get(self.cursor), key) {
            self.step_rank(if key == Key::Left { -1 } else { 1 });
            return;
        }
        // MODES: Left / Right change a room setting; Square on a seat (the
        // host) gives it to the computer or opens it.
        if let (Some((_, Action::ModeSetting(row))), Key::Left | Key::Right) = (rows.get(self.cursor), key) {
            step_setting(&mut self.mode_settings, *row, if key == Key::Left { -1 } else { 1 });
            self.sfx.push(SFX_CURSOR);
            return;
        }
        if let (Some((_, Action::Seat(seat))), Key::Square) = (rows.get(self.cursor), key) {
            self.toggle_computer(*seat);
            return;
        }
        // ALLOW WATCHING: Left / Right turn it on or off.
        if let (Some((_, Action::AllowWatching)), Key::Left | Key::Right) = (rows.get(self.cursor), key) {
            self.act(Action::AllowWatching);
            return;
        }
        // Triangle on a deck: its deck view.
        if key == Key::Triangle {
            match rows.get(self.cursor).map(|r| r.1) {
                Some(Action::PickDeck(i)) => {
                    if let Some(e) = self.deck_entries().into_iter().nth(i) {
                        self.open_preview(e);
                    }
                }
                Some(Action::OpenDeck(i)) => self.preview_custom(i),
                Some(Action::PickComputer(i)) if i > 0 => {
                    if let Some(e) = self.computer_entries().into_iter().nth(i - 1) {
                        self.open_preview(e);
                    }
                }
                _ if matches!(self.screen, Screen::DeckMenu { .. }) => self.act(Action::ViewDeck),
                _ => self.sfx.push(SFX_BUZZER),
            }
            return;
        }
        match key {
            Key::Backspace => {
                if let Some(f) = self.focused_field() {
                    if self.field_mut(f).pop().is_some() {
                        self.sfx.push(SFX_CANCEL);
                    }
                }
            }
            Key::Up | Key::Left if n > 0 => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.sfx.push(SFX_CURSOR);
                }
            }
            Key::Down | Key::Right if n > 0 => {
                if self.cursor + 1 < n {
                    self.cursor += 1;
                    self.sfx.push(SFX_CURSOR);
                }
            }
            Key::Cross if n > 0 => {
                let (_, action) = rows[self.cursor.min(n - 1)];
                self.act(action);
            }
            Key::Circle => self.back(),
            _ => {}
        }
    }

    fn back(&mut self) {
        let action = match &self.screen {
            Screen::NoServer | Screen::Server => Action::Title,
            Screen::Lobby => return,
            Screen::Challenged { .. } => Action::Decline,
            Screen::Sent { .. } => Action::CancelChallenge,
            Screen::RoomOpen => Action::CloseRoom,
            Screen::Result { .. } => Action::Ok,
            Screen::Starting | Screen::AwaitResult { .. } | Screen::WatchStarting => return,
            Screen::ModeRoom => Action::ModeLeave,
            _ => Action::Back,
        };
        self.act(action);
    }

    fn step_rank(&mut self, delta: i32) {
        let Screen::DeckMenu { index } = self.screen else { return };
        let Some(d) = self.library.as_mut().and_then(|l| l.custom.decks.get_mut(index)) else { return };
        let rank = (i32::from(d.leader_rank) + delta).clamp(i32::from(custom_decks::RANK_MIN), i32::from(custom_decks::RANK_MAX)) as u8;
        if rank == d.leader_rank {
            self.sfx.push(SFX_BUZZER);
            return;
        }
        d.leader_rank = rank;
        self.sfx.push(SFX_CURSOR);
        self.save_decks();
        if let Some(choice) = self.choice.clone() {
            self.use_choice(&choice);
        }
    }

    fn lobby_send(&mut self, request: LobbyRequest) {
        if let Some(l) = self.lobby.as_mut() {
            l.send(request);
        }
    }

    /// On the lobby screen with the server still counting this player in a
    /// match (a game that could not join, or left early): it refuses anything
    /// new until that match closes.
    fn held_by_a_match(&self) -> bool {
        let Some(l) = self.lobby.as_ref().filter(|_| self.screen == Screen::Lobby) else { return false };
        l.players.iter().any(|p| Some(&p.name) == l.user.as_ref() && p.state == PlayerState::InMatch)
    }

    fn act(&mut self, action: Action) {
        if self.held_by_a_match()
            && matches!(action, Action::Challenge(_) | Action::CreateRoom | Action::JoinRoom | Action::Computer | Action::Modes)
        {
            self.sfx.push(SFX_BUZZER);
            self.note = Some("wait for your last match to close".into());
            return;
        }
        let ok = |m: &mut Self| m.sfx.push(SFX_CONFIRM);
        match action {
            Action::Title => {
                self.sfx.push(SFX_CANCEL);
                if let Some(mut l) = self.lobby.take() {
                    l.close("left the lobby");
                }
                self.request = Some(MenuRequest::Title);
            }
            Action::Connect => {
                let Ok(config) = self.config.clone() else { return };
                ok(self);
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(friends::connect::<LobbyRequest, LobbyEvent>(&config).map_err(|e| e.to_string()));
                });
                self.connecting = Some(rx);
                self.note = None;
                self.go(Screen::Connecting);
            }
            Action::Back => {
                self.sfx.push(SFX_CANCEL);
                match self.screen {
                    Screen::Connecting | Screen::Account => {
                        self.connecting = None;
                        if let Some(mut l) = self.lobby.take() {
                            l.close("disconnected");
                        }
                        self.go(Screen::Server);
                    }
                    Screen::Form { .. } | Screen::Signing => self.go(Screen::Account),
                    Screen::RoomCode => self.go(Screen::Lobby),
                    Screen::ModeSetup { editing: false } => {
                        self.note = None;
                        self.go(Screen::Lobby);
                    }
                    Screen::ModeSetup { editing: true } => {
                        // The room's own settings stay.
                        if let Some(r) = self.mode_room() {
                            self.mode_settings = r.settings.clone();
                        }
                        self.go(Screen::ModeRoom);
                    }
                    Screen::ModeRoomCode => self.go(Screen::ModeSetup { editing: false }),
                    Screen::Watch => {
                        self.note = None;
                        self.go(Screen::Lobby);
                    }
                    Screen::DeckSelect { ref then } => match then.clone() {
                        Pending::Accept { id, from } => self.go(Screen::Challenged { id, from }),
                        Pending::Copy => self.go(Screen::Decks),
                        _ => self.go(Screen::Lobby),
                    },
                    Screen::Decks => {
                        let signed_in = self.lobby.as_ref().is_some_and(|l| l.user.is_some());
                        let to = if signed_in { Screen::Lobby } else { Screen::Server };
                        self.go(to);
                    }
                    Screen::DeckMenu { .. } => {
                        let at = self.cursor_of_deck();
                        self.go(Screen::Decks);
                        self.cursor = at;
                    }
                    Screen::DeleteDeck { index } => self.open_deck(index),
                    Screen::ComputerSelect => self.choose_deck_then_back(Pending::Computer),
                    Screen::MapSelect { ref then } => match then.clone() {
                        Pending::Computer => self.go(Screen::ComputerSelect),
                        Pending::ModeBoard => {
                            let editing = self.mode_room().is_some();
                            self.go(Screen::ModeSetup { editing });
                        }
                        then => self.choose_deck_then_back(then),
                    },
                    _ => {}
                }
            }
            Action::LogIn | Action::SignUp => {
                ok(self);
                self.password.clear();
                if action == Action::SignUp {
                    // A new account: a new name (log-in keeps the last one).
                    self.name.clear();
                }
                self.note = None;
                self.go(Screen::Form { signup: action == Action::SignUp });
            }
            Action::Field(_) => {
                // Confirm on a field moves to the next row.
                self.cursor += 1;
                self.sfx.push(SFX_CURSOR);
            }
            Action::Submit => {
                let signup = matches!(self.screen, Screen::Form { signup: true });
                let problem = check_username(&self.name).and_then(|_| check_password(&self.password)).err();
                if let Some(p) = problem {
                    self.sfx.push(SFX_BUZZER);
                    self.note = Some(auth_note(p));
                    return;
                }
                let invite_required = self.invite_required();
                if signup && invite_required && normalize_code(&self.invite).len() != 8 {
                    self.sfx.push(SFX_BUZZER);
                    self.note = Some("type the invite code you were given (XXXX-XXXX)".into());
                    return;
                }
                ok(self);
                let (username, password) = (self.name.clone(), self.password.clone());
                self.lobby_send(if signup {
                    let invite = if invite_required { self.invite.clone() } else { String::new() };
                    LobbyRequest::SignUp { username, password, invite }
                } else {
                    LobbyRequest::LogIn { username, password }
                });
                self.resuming = false;
                self.signup = signup;
                self.note = None;
                self.go(Screen::Signing);
            }
            Action::Challenge(i) => {
                let me = self.lobby.as_ref().and_then(|l| l.user.clone()).unwrap_or_default();
                let target = self
                    .lobby
                    .as_ref()
                    .and_then(|l| l.players.iter().filter(|p| p.name != me).nth(i).cloned());
                match target {
                    Some(p) if p.state == PlayerState::Idle => self.choose_deck_then(Pending::Challenge(p.name)),
                    Some(p) => {
                        self.sfx.push(SFX_BUZZER);
                        self.note = Some(cut(&format!("{} is busy", p.name)));
                    }
                    None => self.sfx.push(SFX_BUZZER),
                }
            }
            Action::CreateRoom => self.choose_deck_then(Pending::CreateRoom),
            Action::JoinRoom => self.choose_deck_then(Pending::JoinRoom),
            Action::Join => {
                if self.room_code.len() != ROOM_CODE_LEN {
                    self.sfx.push(SFX_BUZZER);
                    self.note = Some(format!("a room code has {ROOM_CODE_LEN} letters"));
                    return;
                }
                ok(self);
                let code = self.room_code.clone();
                self.lobby_send(LobbyRequest::JoinRoom { code });
                self.go(Screen::Lobby);
            }
            Action::Computer => self.choose_deck_then(Pending::Computer),
            Action::LogOut => {
                self.sfx.push(SFX_CANCEL);
                self.lobby_send(LobbyRequest::LogOut);
                self.forget_session();
                self.password.clear();
                self.go(Screen::Account);
            }
            Action::Accept if self.library.is_some() => {
                if let Screen::Challenged { id, from } = self.screen.clone() {
                    self.choose_deck_then(Pending::Accept { id, from });
                }
            }
            Action::Accept | Action::Decline => {
                if let Screen::Challenged { id, .. } = self.screen {
                    let accept = action == Action::Accept;
                    self.sfx.push(if accept { SFX_CONFIRM } else { SFX_CANCEL });
                    self.lobby_send(LobbyRequest::Answer { id, accept });
                    self.go(Screen::Lobby);
                }
            }
            Action::CancelChallenge => {
                if let Screen::Sent { id, .. } = self.screen {
                    self.sfx.push(SFX_CANCEL);
                    self.lobby_send(LobbyRequest::CancelChallenge { id });
                }
            }
            Action::CloseRoom => {
                self.sfx.push(SFX_CANCEL);
                self.lobby_send(LobbyRequest::LeaveRoom);
            }
            Action::Ok => {
                ok(self);
                self.multi_active = None;
                self.go(Screen::Lobby);
            }
            Action::Decks => {
                ok(self);
                self.note = self.library.as_ref().and_then(|l| l.custom_error.clone()).map(|e| cut(&e));
                self.go(Screen::Decks);
            }
            Action::PickDeck(i) => {
                let Screen::DeckSelect { then } = self.screen.clone() else { return };
                let Some(entry) = self.deck_entries().into_iter().nth(i) else { return };
                if then == Pending::Copy {
                    self.fork(&entry);
                    return;
                }
                match entry.deck {
                    Ok(deck) => {
                        self.deck = Ok(DeckChoice { deck, source: entry.name.clone() });
                        self.remember(&entry.reference);
                        self.after_deck(then);
                    }
                    Err(e) => {
                        self.sfx.push(SFX_BUZZER);
                        self.note = Some(cut(&format!("{} is not ready: {e}", entry.name)));
                    }
                }
            }
            Action::NewDeck => {
                if let Some(e) = self.library.as_ref().and_then(|l| l.custom_error.clone()) {
                    self.sfx.push(SFX_BUZZER);
                    self.note = Some(cut(&e));
                    return;
                }
                match self.library.as_mut().and_then(|l| l.custom.add(CustomDeck::new("NEW DECK"))) {
                    Some(index) => {
                        ok(self);
                        self.save_decks();
                        self.open_deck(index);
                        // Straight to its name.
                        self.cursor = 2;
                    }
                    None => {
                        self.sfx.push(SFX_BUZZER);
                        self.note = Some("no room for another deck".into());
                    }
                }
            }
            Action::CopyDeck => self.choose_deck_then(Pending::Copy),
            Action::OpenDeck(i) => {
                ok(self);
                self.note = None;
                self.open_deck(i);
            }
            Action::EditDeck => {
                if let Screen::DeckMenu { index } = self.screen {
                    ok(self);
                    self.request = Some(MenuRequest::EditDeck(index));
                }
            }
            Action::PickMap(i) => {
                let Screen::MapSelect { then } = self.screen.clone() else { return };
                let Some(map) = self.map_entries().into_iter().nth(i) else { return };
                self.last_map = i;
                self.send_with_map(then, Some(map));
            }
            Action::PickComputer(i) => {
                let choice = match i {
                    0 => Some(ComputerDeck::Random),
                    i => self.computer_entries().into_iter().nth(i - 1).and_then(|e| {
                        let name = if e.source == online_decks::Source::Opponent { e.name.clone() } else { "COMPUTER".into() };
                        e.deck.ok().map(|deck| ComputerDeck::Deck { name, deck })
                    }),
                };
                if choice.is_none() {
                    self.sfx.push(SFX_BUZZER);
                    return;
                }
                self.computer_choice = choice;
                self.choose_map(Pending::Computer);
            }
            Action::ViewDeck => {
                if let Screen::DeckMenu { index } = self.screen {
                    self.preview_custom(index);
                }
            }
            Action::LeaderRank => self.step_rank(1),
            Action::Duplicate => {
                let Screen::DeckMenu { index } = self.screen else { return };
                match self.library.as_mut().and_then(|l| l.custom.duplicate(index)) {
                    Some(copy) => {
                        ok(self);
                        self.save_decks();
                        self.open_deck(copy);
                        self.note = Some(cut(&format!("Copied as {}", self.deck_name)));
                    }
                    None => {
                        self.sfx.push(SFX_BUZZER);
                        self.note = Some("no room for another deck".into());
                    }
                }
            }
            Action::Delete => {
                if let Screen::DeckMenu { index } = self.screen {
                    self.sfx.push(SFX_CONFIRM);
                    self.go(Screen::DeleteDeck { index });
                }
            }
            Action::Watch => {
                ok(self);
                self.note = None;
                if let Some(l) = self.lobby.as_mut() {
                    l.live = None;
                }
                self.ask_for_list();
                self.go(Screen::Watch);
            }
            Action::WatchMatch(i) => {
                let Some(m) = self.live().get(i).cloned() else {
                    self.sfx.push(SFX_BUZZER);
                    return;
                };
                ok(self);
                self.note = None;
                if let Some(l) = self.lobby.as_mut() {
                    l.watch = None;
                }
                self.watch_seats = (m.seats.len() > 2).then(|| m.seats.clone());
                self.lobby_send(LobbyRequest::Watch { match_id: m.match_id });
            }
            Action::AllowWatching => {
                self.allow_watching = !self.allow_watching;
                self.sfx.push(SFX_CURSOR);
                if let Err(e) = save_allow_watching(&self.watching_path, self.allow_watching) {
                    self.note = Some(cut(&format!("the setting was not saved: {e}")));
                }
                let allow = self.allow_watching;
                self.lobby_send(LobbyRequest::AllowSpectators { allow });
            }
            Action::Modes => {
                ok(self);
                self.note = None;
                self.go(Screen::ModeSetup { editing: false });
            }
            Action::ModeSetting(row) => {
                step_setting(&mut self.mode_settings, row, 1);
                self.sfx.push(SFX_CURSOR);
            }
            Action::ModeBoard => self.choose_map(Pending::ModeBoard),
            Action::ModeCreate => self.choose_deck_then(Pending::CreateModeRoom),
            Action::ModeJoin => self.choose_deck_then(Pending::JoinModeRoom),
            Action::ModeJoinCode => {
                if self.room_code.len() != ROOM_CODE_LEN {
                    self.sfx.push(SFX_BUZZER);
                    self.note = Some(format!("a room code has {ROOM_CODE_LEN} letters"));
                    return;
                }
                ok(self);
                let code = self.room_code.clone();
                self.lobby_send(LobbyRequest::JoinModeRoom { code });
                self.note = None;
                self.go(Screen::ModeRoom);
            }
            Action::ModeEdit => {
                ok(self);
                if let Some(r) = self.mode_room() {
                    self.mode_settings = r.settings.clone();
                }
                self.go(Screen::ModeSetup { editing: true });
            }
            Action::ModeApply => {
                ok(self);
                let settings = self.mode_settings.clone();
                self.lobby_send(LobbyRequest::ModeRoomSettings { settings });
                self.go(Screen::ModeRoom);
            }
            Action::Seat(seat) => {
                let me = self.lobby.as_ref().and_then(|l| l.user.clone());
                match self.mode_room().and_then(|r| r.seats.get(usize::from(seat))).cloned() {
                    Some(s) if s.player.is_some() && s.player == me => self.sfx.push(SFX_BUZZER),
                    Some(s) if s.player.is_some() || s.computer => {
                        self.sfx.push(SFX_BUZZER);
                        self.note = Some("that seat is taken".into());
                    }
                    Some(_) => {
                        ok(self);
                        self.lobby_send(LobbyRequest::ModeRoomSeat { seat });
                    }
                    None => self.sfx.push(SFX_BUZZER),
                }
            }
            Action::ModeStart => {
                ok(self);
                self.lobby_send(LobbyRequest::StartModeRoom);
            }
            Action::ModeLeave => {
                self.sfx.push(SFX_CANCEL);
                self.lobby_send(LobbyRequest::LeaveRoom);
                if let Some(l) = self.lobby.as_mut() {
                    l.mode_room = None;
                }
                self.go(Screen::ModeSetup { editing: false });
            }
            Action::ConfirmDelete => {
                let Screen::DeleteDeck { index } = self.screen else { return };
                let removed = self.library.as_mut().and_then(|l| l.custom.delete(index));
                if let Some(d) = removed {
                    self.sfx.push(SFX_CANCEL);
                    self.save_decks();
                    self.note = Some(cut(&format!("{} was deleted", d.name)));
                    if self.choice == Some(DeckRef::Custom { name: d.name }) {
                        self.choice = None;
                    }
                }
                self.go(Screen::Decks);
            }
        }
    }

    /// The opponent's name as the screens show it: their account name, or
    /// the computer as its deck's character ("Pegasus Crawford") when the
    /// server names it.
    pub fn opponent_name(&self) -> Option<String> {
        let r = self.lobby.as_ref()?.ready.as_ref()?;
        Some(r.opponent_title.clone().unwrap_or_else(|| r.opponent.clone()))
    }

    /// The match's names by seat (this player's account name, the
    /// opponent's; both players' while watching), for the duel's HUD.
    pub fn seat_names(&self) -> Option<[String; 2]> {
        if let Some(w) = &self.watching {
            return Some(w.players.clone());
        }
        let lobby = self.lobby.as_ref()?;
        let (me, seat) = (lobby.user.clone()?, lobby.ready.as_ref()?.seat);
        let other = self.opponent_name()?;
        Some(if seat == 0 { [me, other] } else { [other, me] })
    }

    /// MODES: the N-player match's names by seat (the HUD's plates).
    pub fn multi_names(&self) -> Option<Vec<String>> {
        if self.watching.is_some() {
            return self.watch_seats.clone();
        }
        Some(self.multi_active.as_ref()?.seats.iter().map(|s| s.name.clone()).collect())
    }

    /// Back to the deck list of `then` (the cursor on the deck in hand).
    fn choose_deck_then_back(&mut self, then: Pending) {
        let entries = self.deck_entries();
        let at = self.choice.as_ref().and_then(|c| entries.iter().position(|e| &e.reference == c)).unwrap_or(0);
        self.go(Screen::DeckSelect { then });
        self.cursor = at;
    }

    /// The deck view of custom deck `index`.
    fn preview_custom(&mut self, index: usize) {
        let entry = self.custom(index).and_then(|d| self.library.as_ref()?.entry(&DeckRef::Custom { name: d.name.clone() }));
        match entry {
            Some(e) => self.open_preview(e),
            None => self.sfx.push(SFX_BUZZER),
        }
    }

    /// Open the deck view of `entry` (from the screen shown now).
    fn open_preview(&mut self, entry: DeckEntry) {
        self.commit_name();
        let Some(lib) = self.library.as_ref() else { return };
        self.preview = Some(DeckPreview::new(entry, lib));
        self.sfx.push(SFX_CONFIRM);
        let back = Box::new(self.screen.clone());
        let back_cursor = self.cursor;
        self.note = None;
        self.go(Screen::DeckView { back, back_cursor });
    }

    /// Leave the deck view for the screen it was opened from.
    fn close_preview(&mut self) {
        let Screen::DeckView { back, back_cursor } = self.screen.clone() else { return };
        self.preview = None;
        if let Screen::DeckMenu { index } = *back {
            self.open_deck(index);
        } else {
            self.go(*back);
        }
        self.cursor = back_cursor;
    }

    /// COPY TO MY DECKS (and COPY A DECK): a new custom deck with the
    /// entry's cards, leader and rank, "NAME (copy)", opened in the deck
    /// builder. The deck copied is not changed.
    fn fork(&mut self, entry: &DeckEntry) {
        if let Some(e) = self.library.as_ref().and_then(|l| l.custom_error.clone()) {
            self.sfx.push(SFX_BUZZER);
            self.note = Some(cut(&e));
            return;
        }
        match self.library.as_mut().and_then(|l| l.fork(entry)) {
            Some(index) => {
                self.sfx.push(SFX_CONFIRM);
                self.save_decks();
                self.preview = None;
                self.open_deck(index);
                self.note = Some(cut(&format!("Copied as {}", self.deck_name)));
                self.request = Some(MenuRequest::EditDeck(index));
            }
            None => {
                self.sfx.push(SFX_BUZZER);
                self.note = Some("no room for another deck".into());
            }
        }
    }

    /// What Cross does in the deck view: the action of the row it was
    /// opened from.
    fn view_cross(&self) -> Option<&'static str> {
        let Screen::DeckView { back, .. } = &self.screen else { return None };
        match &**back {
            Screen::DeckSelect { then: Pending::Copy } => Some("COPY"),
            Screen::DeckSelect { .. } | Screen::ComputerSelect => Some("CHOOSE"),
            Screen::Decks | Screen::DeckMenu { .. } => Some("EDIT"),
            _ => None,
        }
    }

    fn view_can_copy(&self) -> bool {
        let accepting = matches!(&self.screen, Screen::DeckView { back, .. } if matches!(**back, Screen::DeckSelect { then: Pending::Accept { .. } }));
        !accepting && self.library.as_ref().is_some_and(|l| l.custom_error.is_none())
    }

    /// A key in the deck view.
    fn press_view(&mut self, key: Key) {
        let Some(mut p) = self.preview.take() else {
            self.close_preview();
            return;
        };
        let cards = self.library.as_ref().and_then(|l| l.cards.as_ref());
        // The keys that leave the view (or copy it) put nothing back.
        match key {
            Key::Up | Key::Down => {
                if p.step(if key == Key::Up { -1 } else { 1 }) {
                    // The pane follows the cursor; a leader row without a
                    // leader has none.
                    if p.highlighted().is_none() {
                        p.detail = None;
                    }
                    self.sfx.push(SFX_CURSOR);
                }
            }
            Key::Left | Key::Right if p.detail.is_some() => {
                p.step_page(if key == Key::Left { -1 } else { 1 });
                self.sfx.push(SFX_CURSOR);
            }
            Key::Left | Key::Right | Key::L2 | Key::R2 => {
                p.step_sort(if matches!(key, Key::Left | Key::L2) { -1 } else { 1 }, cards);
                self.sfx.push(SFX_CURSOR);
            }
            Key::Triangle => {
                let opened = p.toggle_detail();
                self.sfx.push(if opened { SFX_CONFIRM } else { SFX_BUZZER });
            }
            Key::Square => {
                self.preview = Some(p.clone());
                if !self.view_can_copy() {
                    self.sfx.push(SFX_BUZZER);
                    self.note = Some(match self.library.as_ref().and_then(|l| l.custom_error.clone()) {
                        Some(e) => cut(&e),
                        None => "answer the challenge first".into(),
                    });
                    return;
                }
                let entry = p.entry.clone();
                self.fork(&entry);
            }
            Key::Cross => {
                self.preview = Some(p.clone());
                let Screen::DeckView { back, .. } = self.screen.clone() else { return };
                let entry = p.entry.clone();
                match *back {
                    Screen::DeckSelect { then: Pending::Copy } => self.fork(&entry),
                    Screen::DeckSelect { .. } => {
                        let i = self.deck_entries().iter().position(|e| e.reference == entry.reference);
                        self.close_preview();
                        if let Some(i) = i {
                            self.act(Action::PickDeck(i));
                        }
                    }
                    Screen::ComputerSelect => {
                        let i = self.computer_entries().iter().position(|e| e.reference == entry.reference);
                        self.close_preview();
                        if let Some(i) = i {
                            self.act(Action::PickComputer(i + 1));
                        }
                    }
                    Screen::Decks | Screen::DeckMenu { .. } => {
                        let index = match &entry.reference {
                            DeckRef::Custom { name } => self.library.as_ref().and_then(|l| l.custom.find(name)),
                            _ => None,
                        };
                        if let Some(index) = index {
                            self.preview = None;
                            self.sfx.push(SFX_CONFIRM);
                            self.open_deck(index);
                            self.request = Some(MenuRequest::EditDeck(index));
                        }
                    }
                    _ => self.sfx.push(SFX_BUZZER),
                }
            }
            Key::Circle => {
                self.sfx.push(SFX_CANCEL);
                if p.detail.take().is_none() {
                    self.close_preview();
                    return;
                }
            }
            _ => {}
        }
        if matches!(key, Key::Up | Key::Down | Key::Left | Key::Right | Key::L2 | Key::R2 | Key::Triangle | Key::Circle) {
            self.preview = Some(p);
        }
    }

    /// The deck view's content.
    fn deck_view(&self) -> Option<DeckView> {
        let (p, lib) = (self.preview.as_ref()?, self.library.as_ref()?);
        let leader = p.deck.leader;
        Some(DeckView {
            name: p.entry.name.clone(),
            tag: p.entry.source.tag().into(),
            leader,
            leader_name: lib.card_name(leader),
            rank: p.deck.leader_rank,
            rank_name: if p.deck.leader_rank == 0 { "no rank".into() } else { lib.rank_name(p.deck.leader_rank) },
            groups: p.groups.clone(),
            cursor: p.cursor,
            first: p.first,
            sort_key: p.sort_key(),
            sort_label: online_decks::sort_label(p.sort_key()).into(),
            detail: p.detail,
            count: p.deck.cards.len(),
            problem: p.entry.deck.as_ref().err().cloned(),
            cross: self.view_cross().map(String::from),
            can_copy: self.view_can_copy(),
        })
    }

    /// The Decks list row of the deck being looked at.
    fn cursor_of_deck(&self) -> usize {
        match self.screen {
            Screen::DeckMenu { index } | Screen::DeleteDeck { index } => index + 2,
            _ => 0,
        }
    }

    /// Once per update: finish a connection attempt, read the lobby and
    /// follow it.
    pub fn update(&mut self) {
        if let Some(rx) = &self.connecting {
            match rx.try_recv() {
                Ok(Ok(conn)) => {
                    self.connecting = None;
                    match LobbyClient::start(conn, self.compat.clone(), concat!("roses-bevy ", env!("CARGO_PKG_VERSION"))) {
                        Ok(client) => {
                            self.lobby = Some(client);
                            self.last_ping = Instant::now();
                            if let Some(saved) = self.saved_session() {
                                self.name = saved.username;
                                self.resuming = true;
                                self.lobby_send(LobbyRequest::ResumeSession { session: saved.session });
                                self.go(Screen::Signing);
                            } else {
                                self.go(Screen::Account);
                            }
                        }
                        Err(e) => {
                            self.note = Some(unreachable_note(&e.to_string()));
                            self.go(Screen::Server);
                        }
                    }
                }
                Ok(Err(e)) => {
                    self.connecting = None;
                    self.sfx.push(SFX_BUZZER);
                    self.note = Some(unreachable_note(&e.to_string()));
                    self.go(Screen::Server);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.connecting = None,
            }
        }
        let Some(lobby) = self.lobby.as_mut() else { return };
        if self.last_ping.elapsed() >= PING_EVERY {
            self.last_ping = Instant::now();
            lobby.send(LobbyRequest::Ping);
        }
        let listed = lobby.players.len();
        let events = lobby.pump();
        let connected = lobby.connected();
        for e in events {
            self.follow(e);
        }
        self.keep_lobby_cursor(listed);
        // (A refusal took the lobby already, and its note says why.)
        if let Some(lost) = self.lobby.take_if(|_| !connected) {
            let why = lost.note.unwrap_or_else(|| "the connection was lost".into());
            if self.screen != Screen::Starting {
                self.note = Some(cut(&why));
                self.go(Screen::Server);
            }
        }
        if let Screen::AwaitResult { since } = self.screen {
            if since.elapsed() > Duration::from_secs(10) {
                self.go(Screen::Lobby);
            }
        }
        if self.screen == Screen::Watch && self.last_list.elapsed() >= WATCH_REFRESH {
            self.ask_for_list();
        }
    }

    /// The list of who is online changed under the LOBBY screen: a cursor on
    /// a button stays on that button, and it never ends up past the last row
    /// (`listed`: the players listed before).
    fn keep_lobby_cursor(&mut self, listed: usize) {
        let now = self.lobby.as_ref().map_or(listed, |l| l.players.len());
        if self.screen != Screen::Lobby || now == listed {
            return;
        }
        // The list has this player too, who has no row.
        if listed > 0 && self.cursor >= listed - 1 {
            self.cursor = (self.cursor + now).saturating_sub(listed);
        }
        self.cursor = self.cursor.min(self.rows().len().saturating_sub(1));
    }

    fn follow(&mut self, e: LobbyEvent) {
        let note_of = |m: &Self| m.lobby.as_ref().and_then(|l| l.note.clone());
        match e {
            LobbyEvent::LobbyRefused { .. } => {
                self.note = note_of(self);
                self.lobby = None;
                self.go(Screen::Server);
            }
            LobbyEvent::SignedIn { username, session } => {
                // This player's "Allow spectators" (the server remembers it
                // only while it runs).
                if self.spectate() {
                    let allow = self.allow_watching;
                    self.lobby_send(LobbyRequest::AllowSpectators { allow });
                }
                self.save_session(&username, &session);
                self.resuming = false;
                self.password.clear();
                self.invite.clear();
                self.note = None;
                if !matches!(self.screen, Screen::Starting) {
                    self.go(Screen::Lobby);
                }
            }
            LobbyEvent::AuthFailed { problem } => {
                if self.resuming {
                    self.resuming = false;
                    self.go(Screen::Account);
                    // A busy server, too many tries from this address, or an
                    // address still to verify: the saved session stays good.
                    if matches!(problem, AuthProblem::Busy | AuthProblem::TooManyAttempts | AuthProblem::EmailNotVerified) {
                        self.note = Some(auth_note(problem));
                    } else {
                        self.forget_session();
                        self.note = Some("please log in".into());
                    }
                } else if self.screen == Screen::Signing {
                    self.sfx.push(SFX_BUZZER);
                    self.note = Some(auth_note(problem));
                    self.screen = Screen::Form { signup: self.signup };
                    self.cursor = 1;
                } else {
                    self.note = Some(auth_note(problem));
                }
            }
            LobbyEvent::SignedOut { elsewhere } => {
                // A log-out of this menu is already on the account screens.
                if elsewhere {
                    self.note = Some("signed in from another place".into());
                    self.go(Screen::Account);
                } else if !matches!(self.screen, Screen::Account | Screen::Form { .. } | Screen::Signing) {
                    self.go(Screen::Account);
                }
            }
            LobbyEvent::Challenged { id, from, map } => {
                // The deck screens give way to a challenge (a typed name is kept).
                let deck_screen = |s: &Screen| {
                    matches!(s, Screen::Decks | Screen::DeckMenu { .. } | Screen::DeleteDeck { .. } | Screen::DeckSelect { then: Pending::Copy })
                };
                let in_decks = (deck_screen(&self.screen)
                    || matches!(&self.screen, Screen::DeckView { back, .. } if deck_screen(back)))
                    && self.lobby.as_ref().is_some_and(|l| l.user.is_some());
                if in_decks {
                    self.preview = None;
                }
                if in_decks {
                    self.commit_name();
                }
                if self.screen == Screen::Lobby || self.screen == Screen::Watch || in_decks {
                    self.sfx.push(SFX_CONFIRM);
                    self.challenge_map = map;
                    self.go(Screen::Challenged { id, from });
                }
            }
            LobbyEvent::ChallengeSent { id, to } => self.go(Screen::Sent { id, to }),
            LobbyEvent::ChallengeClosed { id, .. } => {
                let accepting = |s: &Screen| matches!(s, Screen::DeckSelect { then: Pending::Accept { id: i, .. } } if *i == id);
                let ours = matches!(&self.screen, Screen::Sent { id: i, .. } | Screen::Challenged { id: i, .. } if *i == id)
                    || accepting(&self.screen)
                    || matches!(&self.screen, Screen::DeckView { back, .. } if accepting(back));
                if ours {
                    self.preview = None;
                    self.go(Screen::Lobby);
                }
                self.note = note_of(self).map(|n| cut(&n));
            }
            LobbyEvent::RoomOpened { .. } => self.go(Screen::RoomOpen),
            LobbyEvent::RoomClosed => {
                if self.screen == Screen::RoomOpen {
                    self.go(Screen::Lobby);
                }
            }
            LobbyEvent::MatchReady { .. } => {
                if let Some(ready) = self.lobby.as_ref().and_then(|l| l.ready.clone()) {
                    self.last_opponent = self.opponent_name();
                    self.multi_active = None;
                    self.sfx.push(SFX_CONFIRM);
                    self.go(Screen::Starting);
                    self.request = Some(MenuRequest::Play(ready));
                }
            }
            LobbyEvent::MatchOver { result, .. } => {
                if matches!(self.screen, Screen::AwaitResult { .. }) || result == MatchResult::Cancelled {
                    self.go(Screen::Result { result });
                }
            }
            LobbyEvent::Problem { .. } => {
                self.sfx.push(SFX_BUZZER);
                self.note = note_of(self).map(|n| cut(&n));
                if self.screen == Screen::Watch {
                    self.ask_for_list();
                }
            }
            LobbyEvent::LiveMatches { .. } => {
                if self.screen == Screen::Watch {
                    let n = self.rows().len();
                    self.cursor = self.cursor.min(n.saturating_sub(1));
                }
            }
            LobbyEvent::WatchReady { .. } => {
                let w = self.lobby.as_ref().and_then(|l| l.watch.clone());
                if let (Some(w), Screen::Watch) = (w, &self.screen) {
                    self.sfx.push(SFX_CONFIRM);
                    self.watching = Some(w.clone());
                    self.go(Screen::WatchStarting);
                    self.request = Some(MenuRequest::Watch(w));
                }
            }
            LobbyEvent::ModeRoom { .. } => {
                if self.screen == Screen::ModeRoom {
                    let n = self.rows().len();
                    self.cursor = self.cursor.min(n.saturating_sub(1));
                }
            }
            LobbyEvent::ModeRoomClosed => {
                if matches!(self.screen, Screen::ModeRoom | Screen::ModeSetup { editing: true }) {
                    self.note = Some("the room was closed".into());
                    self.go(Screen::ModeSetup { editing: false });
                }
            }
            LobbyEvent::MultiMatchReady { .. } => {
                if let Some(ready) = self.lobby.as_ref().and_then(|l| l.multi_ready.clone()) {
                    self.last_opponent = None;
                    self.multi_active = Some(ready.clone());
                    self.sfx.push(SFX_CONFIRM);
                    self.note = None;
                    self.go(Screen::Starting);
                    self.request = Some(MenuRequest::PlayMulti(ready));
                }
            }
            LobbyEvent::LobbyWelcome { .. } | LobbyEvent::Players { .. } | LobbyEvent::Pong | LobbyEvent::PasswordChanged => {}
            // Email answers go to the launcher's account page, not the game.
            LobbyEvent::AccountStatus { .. }
            | LobbyEvent::EmailCodeSent
            | LobbyEvent::ResetCodeSent
            | LobbyEvent::PasswordReset
            | LobbyEvent::EmailFailed { .. } => {}
            // Account events (the launcher's page uses them) need nothing here.
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }

    /// The game could not join the match the lobby arranged (or the duel
    /// to watch).
    pub fn match_failed(&mut self, why: &str) {
        self.sfx.push(SFX_BUZZER);
        self.multi_active = None;
        self.note = Some(cut(why));
        if self.watching.take().is_some() && self.lobby.is_some() {
            self.ask_for_list();
            self.go(Screen::Watch);
            return;
        }
        self.go(if self.lobby.is_some() { Screen::Lobby } else { Screen::Server });
    }

    /// Back from the duel: the result (as the server tells it); back from
    /// watching one: the WATCH list.
    pub fn returned(&mut self) {
        if self.watching.take().is_some() {
            if self.lobby.is_some() {
                self.ask_for_list();
                self.go(Screen::Watch);
            } else {
                self.go(Screen::Server);
            }
            return;
        }
        match self.lobby.as_ref().and_then(|l| l.over.clone()) {
            Some((_, result, _)) => self.go(Screen::Result { result }),
            None if self.lobby.is_some() => self.go(Screen::AwaitResult { since: Instant::now() }),
            None => self.go(Screen::Server),
        }
    }

    /// A deck's lines: its leader and rank, its cards (or what it lacks)
    /// and where it comes from.
    fn deck_info(&self, e: &DeckEntry, info: &mut Vec<String>) {
        let Some(lib) = &self.library else { return };
        info.push(cut(&e.name));
        if e.leader == roses_core::card_block::EMPTY_CARD {
            info.push("Leader: none yet".into());
        } else {
            info.push(cut(&format!("Leader: {}", lib.card_name(e.leader))));
        }
        let rank = if e.leader_rank == 0 { "no rank".to_string() } else { format!("rank {}", lib.rank_name(e.leader_rank)) };
        match &e.deck {
            Ok(d) => info.push(cut(&format!("{} cards, leader {rank}", d.cards.len()))),
            Err(p) => info.push(cut(&format!("Not ready: {p}"))),
        }
        info.push(
            match e.source {
                online_decks::Source::Save => "From your save",
                online_decks::Source::Opponent => "The computer opponent's deck",
                online_decks::Source::Custom => "Your custom deck",
            }
            .into(),
        );
    }

    pub fn view(&self) -> View {
        let config = self.config.as_ref().ok();
        let label = config.map(|c| c.label()).unwrap_or_default();
        let me = self.lobby.as_ref().and_then(|l| l.user.clone());
        let mut info: Vec<String> = Vec::new();
        let heading = match &self.screen {
            Screen::NoServer => {
                info.push("No server is set up yet.".into());
                info.push("Ask the server's owner for online.json".into());
                info.push("and put it in the game data folder:".into());
                // The message font has no backslash (it drew "?").
                let path = self.config_path.display().to_string().replace('\\', "/");
                for chunk in path.chars().collect::<Vec<_>>().chunks(LINE_MAX) {
                    info.push(chunk.iter().collect());
                }
                "NO SERVER"
            }
            Screen::Server => {
                if let Some(c) = config {
                    info.push(cut(&format!("Server: {label}")));
                    info.push(cut(&c.server));
                    info.push(
                        match (c.tls, &c.pin) {
                            (true, Some(_)) => "Secure connection (pinned certificate)",
                            (true, None) => "Secure connection",
                            (false, _) => "This computer only (no encryption)",
                        }
                        .into(),
                    );
                }
                match &self.deck {
                    Ok(d) => info.push(cut(&format!("Deck: {}", d.source))),
                    Err(e) => info.push(cut(&format!("No deck: {e}"))),
                }
                "SERVER"
            }
            Screen::Connecting => {
                info.push(cut(&format!("Connecting to {label}...")));
                "CONNECTING"
            }
            Screen::Account => {
                info.push(cut(&format!("Server: {label}")));
                if self.invite_required() {
                    info.push("Log in, or sign up with the invite".into());
                    info.push("code the server's owner gave you.".into());
                } else {
                    info.push("Log in, or sign up with a name".into());
                    info.push("and a password of your own.".into());
                }
                "ACCOUNT"
            }
            Screen::Form { signup } => {
                info.push("Type with the keyboard; Enter moves on.".into());
                if *signup {
                    info.push("Name: 3 to 12 letters or digits.".into());
                    info.push("Password: at least 8 characters.".into());
                    "SIGN UP"
                } else {
                    "LOG IN"
                }
            }
            Screen::Signing => {
                info.push("Signing in...".into());
                "ACCOUNT"
            }
            Screen::Lobby => {
                info.push(cut(&format!("{} on {label}", me.clone().unwrap_or_default())));
                if self.library.is_some() {
                    if let Ok(d) = &self.deck {
                        info.push(cut(&format!("Deck: {}", d.source)));
                    }
                }
                let others = self.rows().iter().filter(|(r, _)| matches!(r, Row::Player(..))).count();
                if self.held_by_a_match() {
                    info.push("Your last match is still closing.".into());
                } else {
                    info.push(if others == 0 { "Nobody else is online.".into() } else { "Choose a player to challenge:".into() });
                }
                "LOBBY"
            }
            Screen::Challenged { from, .. } => {
                info.push(cut(&format!("{from} challenges you to a duel!")));
                if let Some(m) = &self.challenge_map {
                    info.push(cut(&format!("Map: {}", online_decks::map_label(m))));
                }
                "CHALLENGE"
            }
            Screen::Sent { to, .. } => {
                info.push(cut(&format!("Waiting for {to} to answer...")));
                "CHALLENGE"
            }
            Screen::RoomOpen => {
                let code = self.lobby.as_ref().and_then(|l| l.room.clone()).unwrap_or_default();
                info.push(format!("Room code: {code}"));
                info.push("Give this code to your friend.".into());
                info.push("Waiting for them to join...".into());
                "ROOM"
            }
            Screen::RoomCode => {
                info.push("Type the room code your friend gave you.".into());
                "JOIN ROOM"
            }
            Screen::Starting if self.multi_active.is_some() => {
                if let Some(r) = &self.multi_active {
                    info.push(cut(&format!("{} match!", mode_name(r.settings.mode))));
                    let names: Vec<String> = r.seats.iter().map(|s| s.name.clone()).collect();
                    info.extend(name_lines(&names));
                    info.push(format!("You sit at {}.", SEAT_NAMES.get(usize::from(r.seat)).copied().unwrap_or("?")));
                }
                info.push("Waiting for every player...".into());
                "MODES"
            }
            Screen::Starting => {
                let opp = self.opponent_name().unwrap_or_default();
                info.push(cut(&format!("Duel with {opp}!")));
                if let Some(m) = self.lobby.as_ref().and_then(|l| l.ready.as_ref()).and_then(|r| r.map.as_ref()) {
                    info.push(cut(&format!("Map: {}", online_decks::map_label(m))));
                }
                info.push("Waiting for both duelists...".into());
                "DUEL"
            }
            Screen::AwaitResult { .. } => {
                info.push("Waiting for the server's result...".into());
                "RESULT"
            }
            Screen::DeckSelect { then } => {
                let entries = self.deck_entries();
                match entries.get(self.cursor) {
                    Some(e) => {
                        self.deck_info(e, &mut info);
                        info.push("Triangle (D): see its cards".into());
                    }
                    None => info.push("No deck to choose.".into()),
                }
                if *then == Pending::Copy {
                    "COPY A DECK"
                } else {
                    "CHOOSE DECK"
                }
            }
            Screen::Decks => {
                let count = self.library.as_ref().map_or(0, |l| l.custom.decks.len());
                info.push("Your own decks for online play:".into());
                info.push("any card of the game, 40 cards,".into());
                info.push("3 copies at most, and a leader.".into());
                if count == 0 {
                    info.push("Make one with NEW DECK or COPY A DECK.".into());
                } else {
                    info.push("Triangle (D) on a deck: see its cards".into());
                }
                "DECKS"
            }
            Screen::DeckMenu { index } | Screen::DeleteDeck { index } => {
                if let Some(e) = self.custom(*index).and_then(|d| self.library.as_ref()?.entry(&DeckRef::Custom { name: d.name.clone() })) {
                    if let Screen::DeleteDeck { .. } = self.screen {
                        info.push(cut(&format!("Delete {}?", e.name)));
                        info.push("It cannot be brought back.".into());
                    } else {
                        self.deck_info(&e, &mut info);
                    }
                }
                if let Screen::DeleteDeck { .. } = self.screen {
                    "DELETE DECK"
                } else {
                    "DECK"
                }
            }
            Screen::DeckView { .. } => {
                if let Some(p) = &self.preview {
                    info.push(cut(&p.entry.name));
                }
                "VIEW DECK"
            }
            Screen::MapSelect { then } => {
                if let Some(m) = self.map_entries().get(self.cursor) {
                    info.push(cut(&format!("Map: {}", online_decks::map_label(m))));
                }
                info.push(
                    match then {
                        Pending::Challenge(to) => cut(&format!("{to} sees it before answering.")),
                        Pending::CreateRoom => "Whoever joins plays on it.".into(),
                        _ => "The duel is played on it.".into(),
                    },
                );
                "CHOOSE MAP"
            }
            Screen::ComputerSelect => {
                let entries = self.computer_entries();
                match self.cursor.checked_sub(1).and_then(|i| entries.get(i)) {
                    Some(e) => {
                        self.deck_info(e, &mut info);
                        info.push("Triangle (D): see its cards".into());
                    }
                    None => {
                        info.push("The computer's deck:".into());
                        info.push("RANDOM: one of the computer".into());
                        info.push("opponents' decks, chosen by the server.".into());
                    }
                }
                "COMPUTER DECK"
            }
            Screen::Watch => {
                let live = self.live();
                match live.get(self.cursor) {
                    Some(m) => {
                        if m.seats.len() > 2 {
                            info.extend(name_lines(&m.seats));
                        } else {
                            info.push(cut(&Self::live_title(m)));
                        }
                        if let Some(map) = &m.map {
                            info.push(cut(&format!("Map: {}", online_decks::map_label(map))));
                        }
                        let watching = match m.watching {
                            0 => "nobody watching yet".to_string(),
                            1 => "1 watching".to_string(),
                            n => format!("{n} watching"),
                        };
                        info.push(cut(&format!("Round {}, {watching}", m.round)));
                        info.push("You see both sides' moves, but no".into());
                        info.push("hand and no face-down card.".into());
                    }
                    None if self.lobby.as_ref().is_some_and(|l| l.live.is_none()) => info.push("Asking the server...".into()),
                    None if live.is_empty() => {
                        info.push("Nobody is dueling where you can".into());
                        info.push("watch right now. The list updates".into());
                        info.push("by itself.".into());
                    }
                    None => info.push("Choose a duel to watch.".into()),
                }
                "WATCH"
            }
            Screen::WatchStarting => {
                if let Some(w) = &self.watching {
                    match &self.watch_seats {
                        Some(seats) => info.extend(name_lines(seats)),
                        None => info.push(cut(&format!("{} vs {}", w.players[0], w.players[1]))),
                    }
                    if let Some(m) = &w.map {
                        info.push(cut(&format!("Map: {}", online_decks::map_label(m))));
                    }
                }
                info.push("Joining as a spectator...".into());
                info.push("Start (S): stop watching.".into());
                "WATCH"
            }
            Screen::ModeSetup { editing } => {
                info.push("INVENTED MODE: not in the original game.".into());
                match self.mode_settings.mode {
                    RoomMode::FreeForAll => info.push("Four players, each for themselves.".into()),
                    RoomMode::Teams => info.push("Two teams of two; mates sit opposite.".into()),
                }
                info.push("Left / Right change a setting.".into());
                if !*editing {
                    info.push("Empty seats get computer players.".into());
                }
                "MODES"
            }
            Screen::ModeRoom => {
                match self.mode_room() {
                    Some(r) => {
                        info.push(cut(&format!("Room code: {}   host: {}", r.code, r.host)));
                        info.push("INVENTED MODE".into());
                        info.push(cut(&settings_line(&r.settings)));
                        info.push("Cross: sit there".into());
                        if self.is_host() {
                            info.push("Square: computer / open (host)".into());
                        }
                    }
                    None => info.push("Opening the room...".into()),
                }
                "ROOM"
            }
            Screen::ModeRoomCode => {
                info.push("INVENTED MODE".into());
                info.push("Type the code of the room to join.".into());
                "JOIN ROOM"
            }
            Screen::Result { result } if self.multi_active.is_some() => {
                info.push(
                    match result {
                        MatchResult::Won => "You won the match!",
                        MatchResult::Lost => "You lost the match.",
                        MatchResult::Draw => "The match was a draw.",
                        MatchResult::Cancelled => "The match was cancelled.",
                    }
                    .into(),
                );
                if let Some(r) = &self.multi_active {
                    info.push(cut(&format!("{} ({})", mode_name(r.settings.mode), "invented mode")));
                }
                "RESULT"
            }
            Screen::Result { result } => {
                info.push(
                    match result {
                        MatchResult::Won => "You won the duel!",
                        MatchResult::Lost => "You lost the duel.",
                        MatchResult::Draw => "The duel was a draw.",
                        MatchResult::Cancelled => "The duel was cancelled.",
                    }
                    .into(),
                );
                if let Some(o) = &self.last_opponent {
                    info.push(cut(&format!("Opponent: {o}")));
                }
                "RESULT"
            }
        };
        let rows: Vec<Row> = self.rows().into_iter().map(|(r, _)| r).collect();
        let cursor = self.cursor.min(rows.len().saturating_sub(1));
        let map = match &self.screen {
            Screen::MapSelect { .. } => {
                let entries = self.map_entries();
                Some(MapPreview { cells: entries.get(cursor).and_then(|m| self.library.as_ref()?.map_cells(m)) })
            }
            Screen::Challenged { .. } => self
                .challenge_map
                .as_ref()
                .map(|m| MapPreview { cells: self.library.as_ref().and_then(|l| l.map_cells(m)) }),
            _ => None,
        };
        View { heading: heading.into(), info, rows, cursor, note: self.note.clone(), typing: self.wants_text(), deck: self.deck_view(), map }
    }
}

#[cfg(test)]
#[path = "online_menu_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "online_modes_tests.rs"]
mod modes_tests;
