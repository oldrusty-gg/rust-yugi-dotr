//! Wire messages. Clients send intentions ([`ClientMessage::Command`] holds an
//! action-level [`DuelCommand`] that `roses_core::duel::Duel::apply`
//! validates); the server answers with an explicit acknowledgement or
//! rejection and broadcasts each accepted change as a numbered revision with
//! per-seat filtered events and views. Nothing here carries the full duel
//! state, deck order, the random-generator state or the match seed.

use crate::deck::{DeckProblem, DeckSubmission};
use crate::filter::{PlayerView, SeenEvent};
use crate::rules::RulesTables;
use roses_core::duel::ui::DuelUi;
use roses_core::duel::{DuelCommand, Outcome, DUEL_SCHEMA};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Bumped whenever a message shape or meaning changes (2: the seat's deck
/// in `Hello`, deck refusals, the public detail in `PlayerView`; 3: the
/// match options in `Welcome`, presses streamed to the other seat
/// ([`Press`]), the other side's open hand in `PlayerView`, the executable's
/// rules tables in the fingerprint; 4: [`Join::Ticket`], a seat the
/// friends-online lobby arranged). Spectating (a port addition) is not a
/// new version: it only adds optional fields and messages that a server
/// sends to a client that asked for them ([`Join::Watch`], `spectate` in
/// [`ClientMessage::Hello`]), so version-4 clients and servers keep
/// talking to each other unchanged.
pub const PROTOCOL_VERSION: u32 = 4;
/// The faithful original rules as the online profile runs them (the
/// original's 2-player Custom Duel rules with the Leader Ability / Card
/// Display switches of [`MatchOptions`], plus the online policies of
/// [`OnlinePolicy`]). A modded/hybrid mode must use its own id.
pub const RULES_PROFILE: &str = "dotr-pal-original-online-2";

/// What both ends must agree on before a match begins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compat {
    pub protocol: u32,
    pub rules_profile: String,
    /// `roses_core::duel::DUEL_SCHEMA` of the build.
    pub duel_schema: String,
    /// [`data_fingerprint`] of the rules data (card catalogue, fusion and
    /// ritual tables) each side imported from its own disc.
    pub data_fingerprint: String,
    /// Mod set (empty for the original profile), sorted.
    pub mods: Vec<String>,
}

impl Compat {
    /// This build's protocol/rules ids with the given data fingerprint.
    pub fn current(data_fingerprint: impl Into<String>) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            rules_profile: RULES_PROFILE.into(),
            duel_schema: DUEL_SCHEMA.into(),
            data_fingerprint: data_fingerprint.into(),
            mods: Vec::new(),
        }
    }

    /// Whether a client's `self` may join a server's `server`.
    pub fn check(&self, server: &Compat) -> Result<(), Incompatible> {
        let pair = |a: &dyn ToString, b: &dyn ToString| (a.to_string(), b.to_string());
        if self.protocol != server.protocol {
            let (client, server) = pair(&self.protocol, &server.protocol);
            return Err(Incompatible::Protocol { client, server });
        }
        if self.rules_profile != server.rules_profile {
            let (client, server) = pair(&self.rules_profile, &server.rules_profile);
            return Err(Incompatible::RulesProfile { client, server });
        }
        if self.duel_schema != server.duel_schema {
            let (client, server) = pair(&self.duel_schema, &server.duel_schema);
            return Err(Incompatible::DuelSchema { client, server });
        }
        if self.data_fingerprint != server.data_fingerprint {
            let (client, server) = pair(&self.data_fingerprint, &server.data_fingerprint);
            return Err(Incompatible::GameData { client, server });
        }
        if self.mods != server.mods {
            return Err(Incompatible::Mods {
                client: self.mods.clone(),
                server: server.mods.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Incompatible {
    Protocol { client: String, server: String },
    RulesProfile { client: String, server: String },
    DuelSchema { client: String, server: String },
    GameData { client: String, server: String },
    Mods { client: Vec<String>, server: Vec<String> },
}

/// An N-player match's seat as every seat's welcome lists it (a port
/// addition, `crate::lobby` MODES).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatInfo {
    /// The account name, or the computer's shown name.
    pub name: String,
    /// The seat is played by the server's computer player.
    #[serde(default, skip_serializing_if = "is_false")]
    pub computer: bool,
    /// 2v2: 0 for seats 0 and 1 (S + N), 1 for seats 2 and 3 (E + W); FFA:
    /// the seat itself.
    pub team: u8,
}

/// What an N-player match's seats are told in their welcome (a port
/// addition; never sent for a 2-player match).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiWelcome {
    pub seats: Vec<SeatInfo>,
    pub settings: crate::lobby::RoomSettings,
}

/// SHA-256 of the catalogue's canonical JSON (cards, effect entries,
/// power-ups, fusions, rituals) and the executable's rules tables (leader
/// abilities 0x2C26B8, Destiny Draw rows 0x2B7AF0): two imports agree on
/// rules data exactly when their fingerprints match. Only the hash is ever
/// exchanged.
pub fn data_fingerprint(catalogue: &roses_core::duel::Catalogue, tables: &RulesTables) -> String {
    let bytes = serde_json::to_vec(catalogue).expect("catalogue serialises");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    tables.digest(&mut hasher);
    format!("sha256:{:x}", hasher.finalize())
}

/// The 2-player Custom Duel switches the match is played with, decided by
/// the server and announced in [`ServerMessage::Welcome`] (each seat sees
/// both): Leader Ability USE / NOT USE (param 0x1C bit s: the side's
/// leader keeps its abilities) and Card Display OPEN / HIDE (param 0x1D
/// bit s). With OPEN a side's hand and face-down cards are not hidden
/// between two pads, so the other seat receives them; with HIDE they are
/// never sent, and the owner sees them only while holding R2, as on the
/// original's shared screen [F custom-duel NOTES §3, queries `hidden`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchOptions {
    pub leader_ability: [bool; 2],
    pub card_display_open: [bool; 2],
}

impl Default for MatchOptions {
    /// The online default: leader abilities in use (the Custom Duel
    /// default), cards hidden (HIDE: nothing hidden is sent to the other
    /// seat unless the server is told OPEN).
    fn default() -> Self {
        Self {
            leader_ability: [true; 2],
            card_display_open: [false; 2],
        }
    }
}

impl MatchOptions {
    /// The duel's option words [param 0x1C, param 0x1D].
    pub fn words(&self) -> [u32; 2] {
        let bits = |b: [bool; 2]| u32::from(b[0]) | u32::from(b[1]) << 1;
        [bits(self.leader_ability), bits(self.card_display_open)]
    }
}

/// What a streamed press did (the other seat's sound and presentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PressResponse {
    Ignored,
    Moved,
    Refused,
    Commands,
    Unsupported,
}

/// One pad press of the acting seat as the other seat's screen shows it
/// (presentation only, never trusted for rules): in the original's
/// 2-player duel both players watch one screen, so the acting player's
/// cursor, menus, hand row and summon flight are seen by the other. The
/// input state is the seat's input layer after the press with every
/// hidden card id removed by the server ([`crate::presence::sanitize`]).
/// A press that sent intentions is streamed after the server decided them:
/// `through_revision` is the last revision they produced (`base_revision`
/// when none was accepted).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Press {
    pub base_revision: u64,
    pub through_revision: u64,
    pub key: u16,
    pub response: PressResponse,
    pub input: DuelUi,
    /// Updates (25 Hz, two PAL fields each) since the seat's previous streamed press: the other
    /// seat presents this press no sooner after the previous one, so the
    /// press-free stretches between them (a details pane at rest, a cursor
    /// pause) keep their length on its screen. 0 from a client that does not
    /// count (presented at once).
    #[serde(default)]
    pub gap: u16,
}

/// The online profile's explicit pause, timeout and forfeit rules. Times
/// are in server ticks (the TCP server uses milliseconds; tests drive ticks
/// directly, so nothing depends on a rendering frame rate).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnlinePolicy {
    /// While any seat is disconnected the whole match is paused: no command
    /// from either seat is accepted and the turn clock stops. A seat absent
    /// longer than this forfeits.
    pub reconnect_grace: u64,
    /// Time the acting seat has for one turn (paused time excluded). On
    /// expiry the server ends the turn for it (closing an open selection
    /// with Circle/cancel first). `None` = no turn clock.
    pub turn_limit: Option<u64>,
    /// Consecutive expired turns after which the seat forfeits instead.
    pub timeouts_before_forfeit: u8,
}

impl Default for OnlinePolicy {
    /// 60 s grace, 120 s turns, forfeit on the third expired turn in a row
    /// (in milliseconds).
    fn default() -> Self {
        Self {
            reconnect_grace: 60_000,
            turn_limit: Some(120_000),
            timeouts_before_forfeit: 3,
        }
    }
}

/// How a client asks for a seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Join {
    /// A free seat (the given one, or the lowest free).
    Seat { seat: Option<u8> },
    /// Reclaim a seat after a disconnect with the secret token from
    /// [`ServerMessage::Welcome`].
    Resume { token: String },
    /// The seat the lobby arranged (`crate::lobby::LobbyEvent::MatchReady`):
    /// a secret per player and match, valid for the match's life (a client
    /// that lost its resume token rejoins with it). Only a lobby server
    /// (`roses-server hub`) knows tickets; a single-match server refuses
    /// them as a bad token.
    Ticket { ticket: String },
    /// Watch a match (a spectator, no seat) with the ticket from the
    /// lobby's `crate::lobby::LobbyEvent::WatchReady`. Sent only to a
    /// server whose lobby welcome says `spectate`; it answers
    /// [`ServerMessage::Watching`] and the spectator's snapshot (the view of
    /// nobody: no hand, no face-down card) instead of a `Welcome`.
    Watch { ticket: String },
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// The first message of every connection. `deck` is the seat's own deck
    /// (required by a server that takes submitted decks when joining a
    /// seat; ignored on a resume). It is checked with
    /// [`crate::deck::check_deck`] and never sent to the other seat.
    Hello {
        compat: Compat,
        join: Join,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deck: Option<DeckSubmission>,
        /// This client streams its presses ([`ClientMessage::Presence`]);
        /// the other seat then waits for them to present its revisions.
        #[serde(default)]
        presence: bool,
        /// This client reads the spectating messages
        /// ([`ServerMessage::Watchers`]): only then does a seat get the
        /// "N watching" count. Absent from older clients (and left out when
        /// false, so their hello is unchanged on the wire).
        #[serde(default, skip_serializing_if = "is_false")]
        spectate: bool,
    },
    /// An intention. `seq` increases strictly per seat for the whole match
    /// (a repeated or lower `seq` is never executed again); `base_revision`
    /// is the revision the client decided on (a different current revision
    /// rejects it as stale).
    Command {
        seq: u64,
        base_revision: u64,
        command: DuelCommand,
    },
    /// Concede (accepted from either seat at any time; also sequenced).
    Forfeit { seq: u64 },
    /// Ask for a fresh [`ServerMessage::Snapshot`].
    Sync,
    /// A press of the acting seat for the other seat's screen.
    Presence { press: Press },
}

impl ClientMessage {
    /// The first message of a connection (spectating messages understood).
    pub fn hello(compat: Compat, join: Join, deck: Option<DeckSubmission>, presence: bool) -> Self {
        Self::Hello { compat, join, deck, presence, spectate: true }
    }
}

/// Why a match ended or a server-decided change happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    /// A seat's own command (its outcome follows the duel rules).
    Command,
    /// A seat conceded.
    Forfeit,
    /// The turn clock ran out; the server ended the turn.
    TurnTimeout,
    /// Too many expired turns in a row.
    RepeatedTimeouts,
    /// A disconnected seat did not come back within the grace period.
    Abandoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    Seat { seat: u8 },
    Server,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchStatus {
    /// Waiting for both seats to join.
    Waiting,
    Running,
    Paused {
        /// Seats currently disconnected.
        absent: [bool; 2],
    },
    Ended {
        outcome: Outcome,
        cause: Cause,
    },
}

/// Why an intention was not executed. The state and revision are unchanged
/// in every case. Rules refusals carry the core's fixed message; an
/// unsupported interaction is reported without its detail, which can name
/// a hidden card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Rejection {
    NotStarted,
    Paused,
    Finished,
    NotYourTurn,
    Stale { base_revision: u64, revision: u64 },
    InvalidCommand { message: String },
    Illegal { message: String },
    /// The rules port refuses this interaction (not yet implemented).
    Unsupported,
}

/// What the server decided for a `seq` the first time it saw it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    Accepted { revision: u64 },
    Rejected { reason: Rejection },
    /// Too old to remember; it was decided once and is never re-executed.
    Forgotten,
}

/// Everything a seat may know, at one revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: u64,
    /// The highest `seq` of this seat the server has decided.
    pub last_seq: u64,
    pub status: MatchStatus,
    pub view: PlayerView,
    /// Which seats stream their presses (per seat: two in a 2-player
    /// match, the former `[bool; 2]` on the wire).
    #[serde(default = "two_false")]
    pub presence: Vec<bool>,
}

fn two_false() -> Vec<bool> {
    vec![false; 2]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// The handshake succeeded. `resume_token` is secret to this seat.
    Welcome {
        match_id: String,
        seat: u8,
        resume_token: String,
        compat: Compat,
        policy: OnlinePolicy,
        /// The Custom Duel switches of this match (server-decided).
        options: MatchOptions,
        /// An N-player match (a port addition: MODES): its seats and room
        /// settings. Absent for a 2-player match, so that welcome is
        /// unchanged on the wire.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        multi: Option<MultiWelcome>,
    },
    /// The handshake failed; the connection is closed and no seat is taken.
    Refused { reason: Refusal },
    Snapshot { snapshot: Snapshot },
    /// The intention `seq` was executed as revision `revision`.
    Accepted { seq: u64, revision: u64 },
    Rejected {
        seq: u64,
        revision: u64,
        reason: Rejection,
    },
    /// `seq` was already decided (a retransmission): not executed again.
    Duplicate { seq: u64, decision: Decision },
    /// Revision `revision` happened (sent to both seats, in order).
    Update {
        revision: u64,
        actor: Actor,
        cause: Cause,
        events: Vec<SeenEvent>,
        view: PlayerView,
    },
    Paused {
        absent: [bool; 2],
        /// Tick at which the absent seat forfeits.
        deadline: u64,
    },
    Resumed,
    /// The other seat's press (see [`Press`]).
    Presence { seat: u8, press: Press },
    Ended {
        revision: u64,
        outcome: Outcome,
        cause: Cause,
    },
    /// A message the server could not read (the connection may be closed).
    ProtocolError { message: String },
    /// A friends server (`roses-server hub`) turns a connection away
    /// before its first message, when it cannot yet tell a match
    /// connection from a lobby one (too many connecting at once, or full),
    /// with the lobby's refusal; a match client reads it here. The
    /// connection is closed. [`crate::client::ClientState`] files it as
    /// [`Refusal::ServerBusy`] (or the refusal it amounts to).
    LobbyRefused { reason: crate::lobby::LobbyRefusal },
    /// A spectator's handshake succeeded ([`Join::Watch`]): the match, its
    /// players' names by seat and its switches. A spectator's snapshot and
    /// updates follow (views with `spectator` set); with a delay of
    /// `delay_turns` a turn's revisions arrive only once that many later
    /// turns have begun (and all of them at the end). Sent only to
    /// spectators.
    Watching {
        match_id: String,
        names: [String; 2],
        options: MatchOptions,
        #[serde(default)]
        delay_turns: u8,
        /// An N-player match (a port addition): every seat. Sent only to
        /// clients that said `modes` in their lobby hello.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        multi: Option<MultiWelcome>,
    },
    /// How many spectators watch this match now. Sent to spectators and to
    /// the seats whose hello said `spectate` (never to older clients).
    Watchers { count: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Refusal {
    Incompatible { detail: Incompatible },
    SeatTaken,
    MatchFull,
    BadToken,
    MatchOver,
    ExpectedHello,
    /// This server plays submitted decks and the Hello carried none.
    DeckRequired,
    /// The submitted deck breaks the original's deck rules.
    IllegalDeck { problem: DeckProblem },
    /// The server turned the connection away before reading it (too many
    /// players connecting at once, or full): try again in a moment. Never
    /// sent as such: a client files [`ServerMessage::LobbyRefused`] so.
    ServerBusy,
    /// Spectating: the match may not be watched (a player turned
    /// spectators off, or it is over or gone). Sent only to spectators.
    NotWatchable,
    /// Spectating: the match or the server has as many spectators as it
    /// takes. Sent only to spectators.
    WatchersFull,
}

impl Refusal {
    /// What to tell the player.
    pub fn plain(&self) -> String {
        match self {
            Self::Incompatible { .. } => "this game version does not match the server's; update the game".into(),
            Self::SeatTaken | Self::MatchFull => "that match already has its players".into(),
            Self::BadToken => "that match is not on the server any more".into(),
            Self::MatchOver => "that match is over".into(),
            Self::ExpectedHello => "the server did not understand this client".into(),
            Self::DeckRequired => "the server needs your deck".into(),
            Self::IllegalDeck { problem } => format!("your deck breaks the deck rules: {problem}"),
            Self::ServerBusy => "the server is busy; try again in a moment".into(),
            Self::NotWatchable => "that duel cannot be watched".into(),
            Self::WatchersFull => "that duel has as many watching as it can take".into(),
        }
    }
}
