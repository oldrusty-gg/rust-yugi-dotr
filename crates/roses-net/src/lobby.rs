//! The friends-online lobby protocol (ONLINE_PLAN.md "Friends online
//! beta"): accounts (sign-up, open or with an invite code; log-in, session
//! resume, password change; account-only connections for the launcher),
//! who is online, challenges, private rooms by code and an optional match
//! against the server's computer player. A lobby connection is separate
//! from a match connection: when two players are paired the server sends
//! each a [`LobbyEvent::MatchReady`] with a secret ticket, and the client
//! opens its match connection with `Join::Ticket` (the unchanged duel
//! protocol of `crate::protocol`). Both run over the same port: the first
//! message tells the server which kind of connection it is.
//!
//! Match choices (a later addition, still lobby version 1): the challenger,
//! the room's host or the player of the computer may name the board
//! ([`MapChoice`]: one of the game's boards, a map of their own or RANDOM)
//! and, against the computer, its deck ([`ComputerDeck`]). Every new field
//! is optional and left out when unset, so an older client's requests are
//! read as before (the import's first board, the server's computer deck);
//! a server that takes the choices says so in
//! [`LobbyEvent::LobbyWelcome::choices`], and a client sends them only then
//! (an older server would ignore them and play the default board).
//!
//! Spectating (a port addition, also still lobby version 1): a server that
//! offers it says so in [`LobbyEvent::LobbyWelcome::spectate`]; only then
//! does a client send [`LobbyRequest::ListMatches`], [`LobbyRequest::Watch`]
//! and [`LobbyRequest::AllowSpectators`], and only clients that sent one of
//! them ever receive the spectating events, so older clients and older
//! servers are unaffected. Matches from private rooms are not listed and
//! cannot be watched; a player who turns "Allow spectators" off (it is on
//! by default) is never watched.

use crate::deck::DeckSubmission;
use crate::protocol::{Cause, ClientMessage, Compat, Incompatible, MatchOptions, ServerMessage};
use serde::{Deserialize, Serialize};

/// Bumped whenever a lobby message changes shape or meaning.
pub const LOBBY_VERSION: u32 = 1;

// The account part of this lobby (names, passwords, codes, email, the
// sign-in problems) is the platform's account protocol, shared with the
// launcher's Profile page: `platform_net::account`, re-exported here so
// every `roses_net::lobby::...` path keeps working and the wire is the same.
pub use platform_net::account::{
    check_new_username, check_password, check_username, normalize_code, username_key, AccountStatus, AuthProblem,
    EmailProblem, EmailState, CODE_ALPHABET, PASSWORD_MAX, PASSWORD_MIN, RESERVED_NAMES, USERNAME_MAX, USERNAME_MIN,
};
/// Room codes are this long.
pub const ROOM_CODE_LEN: usize = 5;

fn yes() -> bool {
    true
}

/// A board's cells (7 × 7) and the highest terrain code (0..=9, the
/// executable's map table and the map editor's ten terrains).
pub const MAP_CELLS: usize = 49;
pub const TERRAIN_MAX: u8 = 9;
/// The longest name of a player's own map or of a computer deck.
pub const CHOICE_NAME_MAX: usize = 24;

/// The board of a lobby match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MapChoice {
    /// The server picks one of the game's boards.
    Random,
    /// One of the game's boards: the executable's map table (0..0x16 the
    /// campaign's, 0x16.. Custom Duel's VS maps).
    Game { index: u8 },
    /// A player's own map (the map editor's): 49 terrain codes 0..=9.
    Own { name: String, cells: Vec<u8> },
}

fn name_ok(name: &str) -> bool {
    name.chars().count() <= CHOICE_NAME_MAX && name.chars().all(|c| c.is_ascii_graphic() || c == ' ')
}

impl MapChoice {
    /// Whether the server can play it (`game_maps`: boards in its table).
    pub fn check(&self, game_maps: usize) -> Result<(), LobbyProblem> {
        let ok = match self {
            Self::Random => true,
            Self::Game { index } => usize::from(*index) < game_maps,
            Self::Own { name, cells } => name_ok(name) && cells.len() == MAP_CELLS && cells.iter().all(|&c| c <= TERRAIN_MAX),
        };
        if ok { Ok(()) } else { Err(LobbyProblem::BadMap) }
    }
}

/// The computer seat's deck (PLAY COMPUTER).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComputerDeck {
    /// The server picks one of the game's computer opponents' decks.
    Random,
    /// This deck, with its leader and leader rank; `name` is shown for the
    /// computer (a computer opponent's, e.g. "Pegasus Crawford").
    Deck { name: String, deck: DeckSubmission },
}

impl ComputerDeck {
    /// The shape the server checks before the deck rules.
    pub fn check_name(&self) -> Result<(), LobbyProblem> {
        match self {
            Self::Deck { name, .. } if !name_ok(name) => Err(LobbyProblem::BadComputerDeck),
            _ => Ok(()),
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyRequest {
    /// The first message of a lobby connection.
    LobbyHello {
        lobby_version: u32,
        compat: Compat,
        client: String,
        /// An account-only connection (the launcher's Profile page): sign
        /// up, log in, log out and change the password without the game's
        /// rules data (only the protocol must match); it never enters the
        /// lobby, and signing in there does not sign the game out.
        #[serde(default, skip_serializing_if = "is_false")]
        account: bool,
        /// This client plays the N-player modes (a port addition: MODES):
        /// only then is it offered rooms and N-player matches to watch.
        /// Left out when false, so an older server reads the hello as before.
        #[serde(default, skip_serializing_if = "is_false")]
        modes: bool,
    },
    /// Create an account. With an invite code from the server's owner, or
    /// with an empty `invite` when the server has open sign-up
    /// ([`LobbyEvent::LobbyWelcome::invite_required`] false). A reset
    /// invite made for an existing name sets that account's password.
    SignUp {
        username: String,
        password: String,
        #[serde(default)]
        invite: String,
    },
    LogIn { username: String, password: String },
    /// Sign in again with a session token from an earlier sign-in.
    ResumeSession { session: String },
    /// End this session (the token stops working).
    LogOut,
    /// Change the signed-in account's password (the other sessions of the
    /// account end; this one stays).
    ChangePassword { old_password: String, new_password: String },
    Challenge {
        to: String,
        /// The board (none: the server's default).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map: Option<MapChoice>,
    },
    CancelChallenge { id: u64 },
    Answer { id: u64, accept: bool },
    /// Open a private room; whoever joins with its code plays you (on the
    /// host's board).
    CreateRoom {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map: Option<MapChoice>,
    },
    JoinRoom { code: String },
    LeaveRoom,
    /// Play the server's computer player (when the server offers one), on
    /// this board and with this deck for the computer (none: the server's).
    PlayComputer {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map: Option<MapChoice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        computer: Option<ComputerDeck>,
    },
    Ping,
    // ---- spectating (only to a server whose welcome says `spectate`) ----
    /// The live matches that may be watched: [`LobbyEvent::LiveMatches`].
    ListMatches,
    /// Watch one: [`LobbyEvent::WatchReady`] with a ticket for the match
    /// connection (`crate::protocol::Join::Watch`), or a problem.
    Watch { match_id: String },
    /// This player's "Allow spectators" setting (on until said otherwise):
    /// off, their matches are not listed and nobody may watch them (anyone
    /// watching one now is sent away).
    AllowSpectators { allow: bool },
    // ---- email (only to a server whose welcome says `email`) ----------
    // ---- MODES (only to a server whose welcome says `modes`) -----------
    /// Open an N-player room (free-for-all or 2v2) with these settings; the
    /// host takes seat 0 ([`LobbyEvent::ModeRoom`]).
    CreateModeRoom { settings: RoomSettings },
    /// Join the room with this code (the first free seat).
    JoinModeRoom { code: String },
    /// Move to another free seat of the room (2v2: change teams).
    ModeRoomSeat { seat: u8 },
    /// The host: a seat played by the server's computer (`computer`), or
    /// open again.
    ModeRoomComputer { seat: u8, computer: bool },
    /// The host changes the room's settings.
    ModeRoomSettings { settings: RoomSettings },
    /// The host starts the match: every open seat gets a computer player.
    StartModeRoom,
    /// The signed-in account's email state: [`LobbyEvent::AccountStatus`].
    AccountStatus,
    /// Give or change the account's address: a code is mailed to it
    /// ([`LobbyEvent::EmailCodeSent`]); a verified address stays until the
    /// code is entered.
    SetEmail { email: String },
    /// The code from that email (answered with an `AccountStatus`).
    VerifyEmail { code: String },
    /// "Not now" to the launcher's request for an address.
    DismissEmailPrompt,
    /// Signed out: mail a password-reset code to the verified address of
    /// `account` (its name or its address). The answer
    /// ([`LobbyEvent::ResetCodeSent`]) is the same whether or not there is
    /// such an account.
    RequestPasswordReset { account: String },
    /// Signed out: a new password with the code from that email
    /// ([`LobbyEvent::PasswordReset`]; every session of the account ends).
    ResetPassword { account: String, code: String, new_password: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LobbyRefusal {
    LobbyVersion { client: u32, server: u32 },
    Incompatible { detail: Incompatible },
    ServerFull,
    ExpectedHello,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerState {
    Idle,
    /// Waiting in a private room.
    InRoom,
    InMatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerEntry {
    pub name: String,
    pub state: PlayerState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeEnd {
    Accepted,
    Declined,
    Cancelled,
    Expired,
    /// The other player left or started another match.
    Gone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyProblem {
    UnknownPlayer,
    PlayerBusy,
    /// You are in a room, a match or already challenging someone.
    YouAreBusy,
    NotYourself,
    NoSuchChallenge,
    BadRoomCode,
    NoComputer,
    ServerBusy,
    /// The board asked for is not one the server can play.
    BadMap,
    /// The computer's deck breaks the deck rules.
    BadComputerDeck,
    /// Spectating: no such live match (it ended, or never was).
    NoSuchMatch,
    /// Spectating: that match may not be watched (a private room's, or a
    /// player turned spectators off).
    NotWatchable,
    /// Spectating: the match or the server has as many spectators as it
    /// takes.
    WatchersFull,
    /// MODES: settings the server cannot play.
    BadSettings,
    /// MODES: the room is full, or that seat is taken.
    SeatTaken,
    /// MODES: only the room's host may do that.
    NotHost,
}

impl std::fmt::Display for LobbyProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownPlayer => "that player is not online",
            Self::PlayerBusy => "that player is busy",
            Self::YouAreBusy => "finish what you are doing first",
            Self::NotYourself => "you cannot challenge yourself",
            Self::NoSuchChallenge => "that challenge is over",
            Self::BadRoomCode => "no room with that code",
            Self::NoComputer => "this server has no computer player",
            Self::ServerBusy => "the server is full; try later",
            Self::BadMap => "the server cannot play that map",
            Self::BadComputerDeck => "the computer cannot play that deck",
            Self::NoSuchMatch => "that duel is over",
            Self::NotWatchable => "that duel cannot be watched",
            Self::WatchersFull => "too many are watching that duel",
            Self::BadSettings => "the server cannot play those settings",
            Self::SeatTaken => "that seat is taken",
            Self::NotHost => "only the room's host can do that",
        })
    }
}

/// A match result from one player's side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchResult {
    Won,
    Lost,
    Draw,
    /// The match never started (a player did not join in time).
    Cancelled,
}

/// The N-player modes (a port addition: MODES).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomMode {
    /// Four players, every one for themselves.
    #[default]
    FreeForAll,
    /// Two teams of two, team-mates opposite each other.
    Teams,
}

/// The settings of an N-player room (the host's room setup screen). The
/// defaults are the design's (MULTIPLAYER-DESIGN.md §0).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSettings {
    pub mode: RoomMode,
    /// A knocked-out player's cards leave the board (true) or stay as
    /// obstacles.
    #[serde(default = "yes")]
    pub cards_leave: bool,
    /// Every player's starting LP.
    #[serde(default = "start_lp")]
    pub start_lp: u16,
    /// 2v2: team-mates share one LP total.
    #[serde(default)]
    pub shared_lp: bool,
    /// Seconds per turn (0: no turn clock).
    #[serde(default = "turn_seconds")]
    pub turn_seconds: u16,
    /// Others may watch the match.
    #[serde(default = "yes")]
    pub spectators: bool,
    /// The board (none: the server's default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<MapChoice>,
}

fn start_lp() -> u16 {
    roses_core::duel::START_LP
}

fn turn_seconds() -> u16 {
    ROOM_TURN_SECONDS
}

/// The default turn clock of an N-player room (seconds).
pub const ROOM_TURN_SECONDS: u16 = 90;
/// The starting LP a room may set.
pub const ROOM_LP_CHOICES: [u16; 6] = [1000, 2000, 4000, 6000, 8000, 9999];
/// The turn clocks a room may set (seconds; 0: none).
pub const ROOM_TURN_CHOICES: [u16; 5] = [60, 90, 120, 180, 0];

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            mode: RoomMode::FreeForAll,
            cards_leave: true,
            start_lp: roses_core::duel::START_LP,
            shared_lp: false,
            turn_seconds: ROOM_TURN_SECONDS,
            spectators: true,
            map: None,
        }
    }
}

impl RoomSettings {
    /// Whether the server can play these settings.
    pub fn check(&self, game_maps: usize) -> Result<(), LobbyProblem> {
        if let Some(m) = &self.map {
            m.check(game_maps)?;
        }
        if self.start_lp == 0 || self.start_lp > roses_core::duel::MAX_LP || self.turn_seconds > 600 {
            return Err(LobbyProblem::BadSettings);
        }
        Ok(())
    }

    /// The engine's rules for these settings.
    pub fn rules(&self) -> roses_core::duel::MultiRules {
        roses_core::duel::MultiRules {
            mode: match self.mode {
                RoomMode::FreeForAll => roses_core::duel::MultiMode::FreeForAll,
                RoomMode::Teams => roses_core::duel::MultiMode::Teams,
            },
            cards_leave: self.cards_leave,
            shared_lp: self.shared_lp && self.mode == RoomMode::Teams,
            round_limit: roses_core::duel::MULTI_ROUND_LIMIT,
            start_lp: self.start_lp,
        }
    }
}

/// One seat of an N-player room ([`LobbyEvent::ModeRoom`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSeat {
    /// The player sitting there (none: open, or the computer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,
    /// The server's computer player will play this seat.
    #[serde(default, skip_serializing_if = "is_false")]
    pub computer: bool,
    /// 2v2: the seat's team (0: S + N, 1: E + W); FFA: the seat.
    pub team: u8,
}

/// An N-player room as its members see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomInfo {
    pub code: String,
    pub host: String,
    pub settings: RoomSettings,
    /// Four seats: S, N, E, W (seat 0 plays first or not by the seed).
    pub seats: Vec<RoomSeat>,
}

/// A live match that may be watched ([`LobbyEvent::LiveMatches`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveMatch {
    pub match_id: String,
    /// The players by seat as the screens show them (the computer as its
    /// deck's character when the server names it).
    pub players: [String; 2],
    /// The board (none: the server's default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<MapChoice>,
    /// The duel's round (1-based, as the HUD counts).
    pub round: u8,
    /// Spectators watching it now.
    pub watching: u16,
    /// An N-player match (a port addition; listed only to clients that
    /// said `modes`): every seat's name. `players` holds the first two.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seats: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<RoomMode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyEvent {
    LobbyWelcome {
        lobby_version: u32,
        server: String,
        /// The server offers a computer opponent.
        computer: bool,
        /// Sign-up needs an invite code (false: open sign-up, anyone with
        /// the game may make an account). Servers from before open
        /// sign-up do not say, which means an invite is needed.
        #[serde(default = "yes")]
        invite_required: bool,
        /// The server takes [`MapChoice`] and [`ComputerDeck`] (older
        /// servers do not say: no choices are offered).
        #[serde(default, skip_serializing_if = "is_false")]
        choices: bool,
        /// The server sends email (addresses, verification, password
        /// reset): the email requests may be sent. Absent from older servers.
        #[serde(default, skip_serializing_if = "is_false")]
        email: bool,
        /// The server lets players watch live matches (the spectating
        /// requests may be sent). Absent from older servers.
        #[serde(default, skip_serializing_if = "is_false")]
        spectate: bool,
        /// The server offers the N-player modes (a port addition: MODES):
        /// the room requests may be sent. Absent from older servers.
        #[serde(default, skip_serializing_if = "is_false")]
        modes: bool,
    },
    LobbyRefused { reason: LobbyRefusal },
    SignedIn {
        username: String,
        /// Secret: keep it to sign in again without the password.
        session: String,
    },
    AuthFailed { problem: AuthProblem },
    /// The session ended (log-out, or the same account signed in elsewhere).
    SignedOut { elsewhere: bool },
    /// The password was changed (other sessions of the account ended).
    PasswordChanged,
    /// Everyone online (sent on every change).
    Players { players: Vec<PlayerEntry> },
    Challenged {
        id: u64,
        from: String,
        /// The board the challenger chose, shown before answering.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map: Option<MapChoice>,
    },
    ChallengeSent { id: u64, to: String },
    ChallengeClosed { id: u64, end: ChallengeEnd },
    RoomOpened { code: String },
    RoomClosed,
    /// Open a match connection with `Join::Ticket { ticket }` and your deck.
    MatchReady {
        match_id: String,
        seat: u8,
        ticket: String,
        opponent: String,
        options: MatchOptions,
        /// The board the match is played on (a random choice resolved);
        /// none: the server's default board.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map: Option<MapChoice>,
        /// The name to show for the opponent when it is not their account
        /// name (the computer with a computer opponent's deck).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opponent_title: Option<String>,
    },
    MatchOver { match_id: String, result: MatchResult, cause: Option<Cause> },
    Problem { problem: LobbyProblem },
    Pong,
    // ---- spectating (answers to the spectating requests only) -----------
    /// The live matches that may be watched now, newest first.
    LiveMatches { matches: Vec<LiveMatch> },
    /// Open a match connection with `Join::Watch { ticket }` (no deck) to
    /// watch `match_id`: a spectator's view, never a hand or a face-down card.
    WatchReady {
        match_id: String,
        ticket: String,
        players: [String; 2],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map: Option<MapChoice>,
        options: MatchOptions,
    },
    // ---- MODES (answers to the room requests only) ----------------------
    /// The N-player room this player is in, after every change.
    ModeRoom { room: RoomInfo },
    /// The room is gone (left, closed by the host, or its match started).
    ModeRoomClosed,
    /// An N-player match is ready: open a match connection with
    /// `Join::Ticket { ticket }` and your deck. Its welcome carries the
    /// seats and settings too.
    MultiMatchReady {
        match_id: String,
        seat: u8,
        ticket: String,
        seats: Vec<crate::protocol::SeatInfo>,
        settings: RoomSettings,
    },
    // ---- email (answers to the email requests only) -------------------
    AccountStatus { status: AccountStatus },
    /// A verification code was mailed to the address just given.
    EmailCodeSent,
    /// If the account has a verified address, a reset code was mailed to it.
    ResetCodeSent,
    /// The password was reset: log in with the new one.
    PasswordReset,
    EmailFailed { problem: EmailProblem },
}

/// What a client may send on a connection of either kind (the server
/// reads this; a client sends the plain message, which serialises the
/// same way).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClientFrame {
    Match(ClientMessage),
    Lobby(LobbyRequest),
}

/// What the server sends on a connection of either kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerFrame {
    Match(ServerMessage),
    Lobby(LobbyEvent),
}

#[cfg(test)]
mod choice_tests {
    use super::*;
    use serde_json::{from_str, json, to_value};

    /// The lobby messages as a server or client from before the match
    /// choices knew them (only the variants that changed).
    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum OldRequest {
        Challenge { to: String },
        CreateRoom,
        PlayComputer,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum OldEvent {
        LobbyWelcome { lobby_version: u32, server: String, computer: bool, invite_required: bool },
        Challenged { id: u64, from: String },
        MatchReady { match_id: String, seat: u8, ticket: String, opponent: String, options: MatchOptions },
    }

    /// An older client's requests read as before (no choices), and a new
    /// client without choices sends exactly what an older server reads.
    #[test]
    fn old_clients_and_old_servers_keep_working() {
        for (old, new) in [
            (json!({"type": "create_room"}), LobbyRequest::CreateRoom { map: None }),
            (json!({"type": "play_computer"}), LobbyRequest::PlayComputer { map: None, computer: None }),
            (json!({"type": "challenge", "to": "Kaiba"}), LobbyRequest::Challenge { to: "Kaiba".into(), map: None }),
        ] {
            assert_eq!(serde_json::from_value::<LobbyRequest>(old.clone()).unwrap(), new);
            assert_eq!(to_value(&new).unwrap(), old, "a request without choices is the old message");
            serde_json::from_value::<OldRequest>(to_value(&new).unwrap()).unwrap();
        }
        // An older server's events: no choices, no board, no title.
        let welcome: LobbyEvent =
            from_str(r#"{"type":"lobby_welcome","lobby_version":1,"server":"S","computer":true,"invite_required":false}"#).unwrap();
        assert!(matches!(welcome, LobbyEvent::LobbyWelcome { choices: false, .. }));
        let ready: LobbyEvent = from_str(
            r#"{"type":"match_ready","match_id":"m","seat":1,"ticket":"t","opponent":"COMPUTER","options":{"leader_ability":[true,true],"card_display_open":[false,false]}}"#,
        )
        .unwrap();
        assert!(matches!(ready, LobbyEvent::MatchReady { map: None, opponent_title: None, .. }));
        // A new server's events are read by an older client (it ignores the
        // new fields).
        let events = [
            LobbyEvent::LobbyWelcome { lobby_version: 1, server: "S".into(), computer: true, invite_required: true, choices: true, email: false, spectate: true, modes: false },
            LobbyEvent::Challenged { id: 3, from: "Yugi".into(), map: Some(MapChoice::Game { index: 0x17 }) },
            LobbyEvent::MatchReady {
                match_id: "m".into(),
                seat: 0,
                ticket: "t".into(),
                opponent: "COMPUTER".into(),
                options: MatchOptions::default(),
                map: Some(MapChoice::Own { name: "MY MAP 1".into(), cells: vec![7; MAP_CELLS] }),
                opponent_title: Some("Pegasus Crawford".into()),
            },
        ];
        for e in events {
            let old: OldEvent = serde_json::from_value(to_value(&e).unwrap()).unwrap();
            if let OldEvent::MatchReady { opponent, .. } = old {
                assert_eq!(opponent, "COMPUTER");
            }
        }
    }

    #[test]
    fn the_server_checks_the_map_and_the_computer_name() {
        assert_eq!(MapChoice::Random.check(0), Ok(()));
        assert_eq!(MapChoice::Game { index: 45 }.check(46), Ok(()));
        assert_eq!(MapChoice::Game { index: 46 }.check(46), Err(LobbyProblem::BadMap));
        let own = |cells: Vec<u8>| MapChoice::Own { name: "MY MAP 2".into(), cells };
        assert_eq!(own(vec![9; 49]).check(0), Ok(()));
        assert_eq!(own(vec![10; 49]).check(0), Err(LobbyProblem::BadMap));
        assert_eq!(own(vec![1; 48]).check(0), Err(LobbyProblem::BadMap));
        let long = MapChoice::Own { name: "x".repeat(CHOICE_NAME_MAX + 1), cells: vec![0; 49] };
        assert_eq!(long.check(0), Err(LobbyProblem::BadMap));
        let deck = crate::deck::DeckSubmission { leader: 1, leader_rank: 1, cards: vec![] };
        assert_eq!(ComputerDeck::Deck { name: "Pegasus\nCrawford".into(), deck }.check_name(), Err(LobbyProblem::BadComputerDeck));
        assert_eq!(ComputerDeck::Random.check_name(), Ok(()));
    }
}

/// The account messages of this lobby are the platform's account protocol
/// (`platform_net::account`, what the launcher's Profile page speaks): the
/// same JSON both ways.
#[cfg(test)]
mod account_wire_tests {
    use super::*;
    use platform_net::account::{AccountEvent, AccountRefusal, AccountRequest};
    use serde_json::{json, to_value};

    #[test]
    fn the_account_requests_are_the_lobby_requests() {
        assert_eq!(LOBBY_VERSION, platform_net::account::LOBBY_VERSION);
        let compat = Compat::current("");
        let game = json!({ "compat": compat }).as_object().unwrap().clone();
        let pairs = [
            (
                LobbyRequest::LobbyHello { lobby_version: 1, compat: compat.clone(), client: "l".into(), account: true, modes: false },
                AccountRequest::LobbyHello { lobby_version: 1, client: "l".into(), account: true, game },
            ),
            (
                LobbyRequest::SignUp { username: "Yugi".into(), password: "p".into(), invite: "I".into() },
                AccountRequest::SignUp { username: "Yugi".into(), password: "p".into(), invite: "I".into() },
            ),
            (LobbyRequest::LogIn { username: "a".into(), password: "b".into() }, AccountRequest::LogIn { username: "a".into(), password: "b".into() }),
            (LobbyRequest::ResumeSession { session: "s".into() }, AccountRequest::ResumeSession { session: "s".into() }),
            (LobbyRequest::LogOut, AccountRequest::LogOut),
            (
                LobbyRequest::ChangePassword { old_password: "o".into(), new_password: "n".into() },
                AccountRequest::ChangePassword { old_password: "o".into(), new_password: "n".into() },
            ),
            (LobbyRequest::Ping, AccountRequest::Ping),
            (LobbyRequest::AccountStatus, AccountRequest::AccountStatus),
            (LobbyRequest::SetEmail { email: "e".into() }, AccountRequest::SetEmail { email: "e".into() }),
            (LobbyRequest::VerifyEmail { code: "c".into() }, AccountRequest::VerifyEmail { code: "c".into() }),
            (LobbyRequest::DismissEmailPrompt, AccountRequest::DismissEmailPrompt),
            (LobbyRequest::RequestPasswordReset { account: "a".into() }, AccountRequest::RequestPasswordReset { account: "a".into() }),
            (
                LobbyRequest::ResetPassword { account: "a".into(), code: "c".into(), new_password: "n".into() },
                AccountRequest::ResetPassword { account: "a".into(), code: "c".into(), new_password: "n".into() },
            ),
        ];
        for (lobby, account) in pairs {
            assert_eq!(to_value(&lobby).unwrap(), to_value(&account).unwrap());
            let read: LobbyRequest = serde_json::from_value(to_value(&account).unwrap()).unwrap();
            assert_eq!(read, lobby, "the server reads the launcher's request");
        }
    }

    #[test]
    fn the_lobby_events_read_as_account_events() {
        let status = AccountStatus {
            state: EmailState::Unverified,
            email: None,
            pending_email: Some("a@b.co".into()),
            prompt_email: false,
            verification_required: true,
            supporter: 0,
        };
        let pairs = [
            (
                LobbyEvent::LobbyRefused { reason: LobbyRefusal::LobbyVersion { client: 2, server: 1 } },
                AccountEvent::LobbyRefused { reason: AccountRefusal::LobbyVersion { client: 2, server: 1 } },
            ),
            (
                LobbyEvent::LobbyRefused { reason: LobbyRefusal::Incompatible { detail: Incompatible::Protocol { client: "3".into(), server: "4".into() } } },
                AccountEvent::LobbyRefused { reason: AccountRefusal::Incompatible { detail: json!({"kind": "protocol", "client": "3", "server": "4"}) } },
            ),
            (LobbyEvent::LobbyRefused { reason: LobbyRefusal::ServerFull }, AccountEvent::LobbyRefused { reason: AccountRefusal::ServerFull }),
            (LobbyEvent::SignedIn { username: "Yugi".into(), session: "s".into() }, AccountEvent::SignedIn { username: "Yugi".into(), session: "s".into() }),
            (LobbyEvent::AuthFailed { problem: AuthProblem::UsernameTaken }, AccountEvent::AuthFailed { problem: AuthProblem::UsernameTaken }),
            (LobbyEvent::SignedOut { elsewhere: true }, AccountEvent::SignedOut { elsewhere: true }),
            (LobbyEvent::PasswordChanged, AccountEvent::PasswordChanged),
            (LobbyEvent::Pong, AccountEvent::Pong),
            (LobbyEvent::AccountStatus { status: status.clone() }, AccountEvent::AccountStatus { status }),
            (LobbyEvent::EmailCodeSent, AccountEvent::EmailCodeSent),
            (LobbyEvent::ResetCodeSent, AccountEvent::ResetCodeSent),
            (LobbyEvent::PasswordReset, AccountEvent::PasswordReset),
            (LobbyEvent::EmailFailed { problem: EmailProblem::TooSoon }, AccountEvent::EmailFailed { problem: EmailProblem::TooSoon }),
            (LobbyEvent::RoomClosed, AccountEvent::Other),
        ];
        for (lobby, account) in pairs {
            let read: AccountEvent = serde_json::from_value(to_value(&lobby).unwrap()).unwrap();
            assert_eq!(read, account);
        }
        let welcome = LobbyEvent::LobbyWelcome { lobby_version: 1, server: "S".into(), computer: true, invite_required: false, choices: true, email: true, spectate: true, modes: false };
        let AccountEvent::LobbyWelcome { lobby_version: 1, server, invite_required: false, email: true, game } =
            serde_json::from_value(to_value(&welcome).unwrap()).unwrap()
        else {
            panic!("the welcome reads")
        };
        assert_eq!((server.as_str(), game.get("computer")), ("S", Some(&json!(true))));
    }
}
