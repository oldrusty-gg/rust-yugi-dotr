//! The friends-online server (`roses-server hub`): one process, one port,
//! any number of lobby connections and matches. A connection's first
//! message decides what it is:
//!
//! - `LobbyHello` (roses_net::lobby): a lobby connection. Accounts
//!   ([`crate::accounts`]: sign-up with an invite code or, with
//!   [`HubSettings::open_signup`], without one; log-in, session resume,
//!   password change; Argon2 runs on worker threads so the loop never
//!   stalls), then
//!   the [`Lobby`]: who is online, challenges, rooms by code, the optional
//!   computer opponent.
//! - `Hello` with `Join::Ticket` (or `Join::Resume`): a seat of a match the
//!   lobby arranged. Each match is an [`OnlineMatch`] exactly as the
//!   single-match server runs it (decks submitted at join, server-decided
//!   options, pause/forfeit policy, replay).
//!
//! Single-threaded polling like [`crate::serve`]; time is milliseconds
//! since start. Limits: [`Limits`] per connection (the computer's internal
//! connections are unlimited), at most `max_connections` connections
//! (`max_connections_per_address` from one address) and
//! [`MAX_PENDING`] waiting for their first message ([`HANDSHAKE_MS`]),
//! signed-out lobby connections closed after [`SIGNED_OUT_MS`], failed
//! sign-ins limited per address, per name from one address and per name
//! from all addresses ([`Attempts`]), open sign-ups limited per address and
//! in total ([`SignUps`]), reserved names refused
//! (`roses_net::lobby::check_new_username`) and the server's
//! [`NameFilter`] applied to new names. A `LobbyHello` with `account`
//! set is an account-only connection (the launcher's Profile page): only
//! the protocol must match, it never enters the lobby, and signing in
//! there leaves the account's game connection signed in.
//!
//! Spectating (a port addition): `ListMatches` lists the running matches
//! that may be watched (not a private room's, not one of a player who
//! turned "Allow spectators" off), `Watch` gives a secret watch ticket and
//! a match connection with `Join::Watch` becomes a spectator: it gets the
//! match's spectator log (the view of nobody, filtered once per match by
//! [`OnlineMatch`], after the optional delay of
//! [`HubSettings::spectate_delay_turns`]) and nothing it sends changes the
//! match. At most [`HubSettings::max_spectators_per_match`] per match and
//! [`HubSettings::max_spectators`] on the server. The server-only replays
//! (`--replay-dir`, with the seed and both decks) are never served.
//!
//! MODES (a port addition, MULTIPLAYER-DESIGN.md): a lobby client that says
//! `modes` in its hello may open an N-player room (free-for-all or 2v2:
//! [`crate::lobby`]'s mode rooms) with the room settings (knocked-out
//! cards leave or stay, starting LP, separate or shared team LP, the turn
//! clock, spectators); the host starts it and every open seat gets the
//! server's computer player. The match is a [`MultiMatch`]; its players
//! join with tickets like a 2-player match. Older clients never see any of
//! it (no room, no N-player match in the watch list).

use crate::accounts::{self, Accounts, MatchRow, PasswordCost, User};
use crate::lobby::{Action, ConnId, Lobby};
use crate::multi::{MultiMatch, MultiSeatPlan};
use crate::name_filter::NameFilter;
use crate::online::OnlineMatch;
use crate::replay::MatchSetup;
use crate::serve::{Budget, Limits, HANDSHAKE_MS, MAX_PENDING};
use roses_core::duel::Catalogue;
use roses_net::lobby::{
    ComputerDeck, LiveMatch, MapChoice,
    check_new_username, check_password, check_username, username_key, AuthProblem, ClientFrame, LobbyEvent, LobbyProblem, LobbyRefusal,
    LobbyRequest, MatchResult, ServerFrame, LOBBY_VERSION, PASSWORD_MAX,
};
use roses_net::transport::{Connection, LoopbackListener, Received};
use roses_net::{
    data_fingerprint, ClientMessage, Compat, DeckSubmission, Join, MatchOptions, OnlinePolicy, Refusal, RulesTables,
    ServerMessage,
};
use std::any::Any;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io;
use std::net::IpAddr;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

// Email addresses, verification and password reset (platform-accounts);
// a child module so it can use the hub's private state.
#[path = "email_hub.rs"]
pub mod email_hub;

pub type HubConn = Box<dyn Connection<ServerFrame, ClientFrame> + Send>;

/// A source of new connections and the peer's address (none for internal
/// ones).
pub trait HubAcceptor {
    fn accept(&mut self) -> io::Result<Option<(HubConn, Option<IpAddr>)>>;
    /// Lines for the server log.
    fn take_notes(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// Unencrypted, loopback only (the local test path and a server played on
/// the same PC).
impl HubAcceptor for LoopbackListener {
    fn accept(&mut self) -> io::Result<Option<(HubConn, Option<IpAddr>)>> {
        let Some((stream, peer)) = self.accept_stream()? else { return Ok(None) };
        let conn = roses_net::transport::FramedTcp::<ServerFrame, ClientFrame>::new(stream)?
            .with_max_frame(self.max_frame())
            .with_send_queue(SEND_QUEUE_BYTES);
        Ok(Some((Box::new(conn), Some(peer.ip()))))
    }
}

/// Connections handed over in memory (tests).
impl HubAcceptor for mpsc::Receiver<HubConn> {
    fn accept(&mut self) -> io::Result<Option<(HubConn, Option<IpAddr>)>> {
        Ok(self.try_recv().ok().map(|c| (c, None)))
    }
}

/// Bytes a connection may leave unread before it is cut off: sends are
/// queued and written without waiting, so one player who stops reading
/// (a stalled network) never holds up everyone else's matches.
pub const SEND_QUEUE_BYTES: usize = 1 << 20;

/// A signed-out lobby connection is closed after this long (ms).
pub const SIGNED_OUT_MS: u64 = 10 * 60_000;
/// A match whose seats have not both joined by then is cancelled (ms).
pub const START_MS: u64 = 120_000;
/// An ended match is dropped this long after its end, or once both seats
/// have left (after [`crate::serve::LINGER`]) (ms).
pub const ENDED_KEEP_MS: u64 = 60_000;
/// Auth jobs (Argon2) waiting at once; more are told the server is busy.
pub const MAX_AUTH_JOBS: usize = 16;
/// Failed sign-ins allowed in [`ATTEMPT_WINDOW_MS`]: per address (any
/// names), per name from one address, and per name from all addresses
/// together. The per-name limit counts each address separately, so a
/// stranger who knows a player's name can only lock themselves out, not
/// the player; the much higher all-addresses limit only stops a guessing
/// attack spread over many addresses.
pub const ATTEMPTS_PER_ADDRESS: usize = 20;
pub const ATTEMPTS_PER_NAME: usize = 8;
pub const ATTEMPTS_PER_NAME_ALL_ADDRESSES: usize = 64;
pub const ATTEMPT_WINDOW_MS: u64 = 15 * 60_000;
/// Default for [`HubSettings::max_connections_per_address`]: a player uses
/// two or three connections at once (lobby, match, the launcher's account
/// page), so this leaves room for a household or a small club behind one
/// address while one host cannot take every slot of `max_connections`.
pub const CONNECTIONS_PER_ADDRESS: usize = 16;
/// Open sign-ups (no invite) allowed per address, and on the whole server,
/// in [`SIGNUP_WINDOW_MS`].
pub const SIGNUPS_PER_ADDRESS: usize = 5;
pub const SIGNUPS_TOTAL: usize = 60;
pub const SIGNUP_WINDOW_MS: u64 = 3_600_000;
/// The shortest time between two lists of who is online (ms) with
/// `online` players in the lobby: none up to 16 players, then growing with
/// the square (64 ms at 128, 256 ms at 256, 1 s at 512), at most 2 s.
pub fn players_interval_ms(online: usize) -> u64 {
    ((online * online) as u64 / 256).min(2_000)
}

/// The computer opponent's name in the lobby and the match log.
pub const COMPUTER_NAME: &str = "COMPUTER";
/// Hub passes in a row that may panic before the server gives up (see `run`).
const MAX_FAILED_PASSES: u32 = 1000;

/// What a caught panic said, for the log.
fn panic_text(panic: &(dyn Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("no message")
}

/// Tests: names whose match or lobby connection panics inside the hub.
#[cfg(test)]
pub(crate) static TEST_PANICS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[cfg(test)]
fn test_panic(name: &str) {
    let listed = TEST_PANICS.lock().unwrap().iter().any(|n| n == name);
    assert!(!listed, "a test panic for {name}");
}
/// Defaults of [`HubSettings::max_spectators_per_match`] and
/// [`HubSettings::max_spectators`].
pub const SPECTATORS_PER_MATCH: usize = 8;
pub const SPECTATORS_TOTAL: usize = 256;

/// The rules data every match is played with.
#[derive(Clone)]
pub struct HubData {
    pub cards: Catalogue,
    pub tables: RulesTables,
    /// The board, first side and rules of a match from its seed (the
    /// import's first duel); the seats' decks replace its decks.
    pub setup: Arc<dyn Fn(u64) -> MatchSetup + Send + Sync>,
    /// The computer opponent's deck (none: no computer opponent).
    pub computer_deck: Option<DeckSubmission>,
    /// The game's boards (the executable's map table, 49 terrain codes
    /// each): `MapChoice::Game` indexes it, RANDOM picks from it (none:
    /// only own maps and the default board).
    pub maps: Vec<Vec<u8>>,
    /// The computer opponents' decks with their names (RANDOM's choices
    /// for the computer's deck; none: the default computer deck).
    pub computer_decks: Vec<(String, DeckSubmission)>,
    /// The executable's fusion and equip tables for the N-player modes'
    /// computer players (none: they plan with the catalogue's).
    pub computer_tables: Option<Arc<roses_core::duel::ai::weevil::Tables>>,
}

impl HubData {
    /// Invented test cards (smoke tests; `--synthetic`).
    pub fn synthetic(computer: bool) -> Self {
        use roses_net::synthetic;
        Self {
            cards: synthetic::catalogue(),
            tables: RulesTables::empty(),
            setup: Arc::new(|seed| MatchSetup {
                seed,
                terrain: synthetic::board(),
                leaders: synthetic::leaders(),
                deck_sources: synthetic::decks(),
                first: 0,
                leader_abilities: [vec![], vec![]],
                custom_rules: None,
                leader_ranks: [0; 2],
                destiny_rows: None,
            }),
            computer_deck: computer.then(|| synthetic::submissions()[1].clone()),
            // Two invented boards (all NORMAL, all code 3).
            maps: vec![synthetic::board(), vec![3; roses_net::lobby::MAP_CELLS]],
            computer_decks: vec![("Synthetic Pegasus".into(), synthetic::submissions()[1].clone())],
            computer_tables: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HubSettings {
    pub name: String,
    pub policy: OnlinePolicy,
    pub options: MatchOptions,
    pub limits: Limits,
    pub max_connections: usize,
    pub max_matches: usize,
    /// Each match's server-only replay is written here (`<match id>.json`).
    pub replay_dir: Option<PathBuf>,
    pub password_cost: PasswordCost,
    pub auth_workers: usize,
    pub start_ms: u64,
    /// Anyone with the game may sign up without an invite code (limited by
    /// [`SignUps`]); invite codes (and password-reset codes) still work.
    pub open_signup: bool,
    /// Connections at once from one address (0: no limit). Loopback
    /// addresses are not limited (the server's own PC, tests, load tests).
    pub max_connections_per_address: usize,
    /// Names refused for new accounts (offensive words, impersonation).
    pub name_filter: Arc<NameFilter>,
    /// Replays in `replay_dir` older than this many days are deleted
    /// (checked hourly; none: kept until the operator deletes them).
    pub replay_keep_days: Option<u64>,
    /// Spectators of one match, and on the whole server (0: nobody may
    /// watch).
    pub max_spectators_per_match: usize,
    pub max_spectators: usize,
    /// Spectators see a turn's revisions only once this many later turns
    /// have begun (0: at once).
    pub spectate_delay_turns: u8,
    /// MODES: milliseconds between two commands of a computer player in an
    /// N-player match (so the others see it play; tests: 0).
    pub multi_computer_delay_ms: u64,
    /// Rehearsals only (`--match-seed`, never on a public server): every
    /// 2-player match uses this seed and keeps the room's host (or the
    /// challenger) at seat 0, so a duel between the same decks on the same
    /// map with the same moves repeats exactly. None: an unpredictable
    /// seed and a random seat order. MODES rooms' N-player matches use it
    /// as well (their seats are the room's in either case).
    pub fixed_seed: Option<u64>,
}

impl Default for HubSettings {
    fn default() -> Self {
        Self {
            name: "Roses friends server".into(),
            policy: OnlinePolicy::default(),
            options: MatchOptions::default(),
            limits: Limits::DEFAULT,
            max_connections: 512,
            max_matches: 100,
            replay_dir: None,
            password_cost: PasswordCost::SERVER,
            auth_workers: 2,
            start_ms: START_MS,
            open_signup: false,
            max_connections_per_address: CONNECTIONS_PER_ADDRESS,
            name_filter: Arc::new(NameFilter::builtin()),
            replay_keep_days: None,
            max_spectators_per_match: SPECTATORS_PER_MATCH,
            max_spectators: SPECTATORS_TOTAL,
            spectate_delay_turns: 0,
            multi_computer_delay_ms: 350,
            fixed_seed: None,
        }
    }
}

#[derive(Debug)]
enum Role {
    Pending,
    Lobby {
        user: Option<SignedIn>,
        /// An Argon2 job of this connection is running.
        authing: bool,
        /// An account-only connection (never in the lobby).
        account: bool,
    },
    Seat {
        match_id: String,
        seat: u8,
    },
    /// A spectator of a match (no seat).
    Watch {
        match_id: String,
    },
    /// MODES: a seat of an N-player match.
    MultiSeat {
        match_id: String,
        seat: u8,
    },
    /// MODES: a spectator of an N-player match.
    MultiWatch {
        match_id: String,
    },
}

/// MODES: an N-player match the hub hosts.
struct HostedMulti {
    game: MultiMatch,
    /// Account names by seat ([`COMPUTER_NAME`] for a computer seat).
    players: Vec<String>,
    tickets: Vec<String>,
    seats: Vec<Option<ConnId>>,
    created: u64,
    created_wall: i64,
    ended_at: Option<u64>,
    /// Listed for spectators (the room allows them).
    listed: bool,
    spectators: Vec<ConnId>,
    /// By seat: knocked out and gone, so given back to the lobby before the
    /// end (the end no longer speaks for them; they may be in another match).
    released: Vec<bool>,
}

#[derive(Clone, Debug)]
struct SignedIn {
    id: i64,
    name: String,
    session: String,
}

struct Conn {
    conn: HubConn,
    role: Role,
    opened: u64,
    budget: Budget,
    peer: Option<IpAddr>,
}

struct Hosted {
    game: OnlineMatch,
    /// Account names by seat ([`COMPUTER_NAME`] for the computer).
    players: [String; 2],
    tokens: [String; 2],
    tickets: [String; 2],
    seats: [Option<ConnId>; 2],
    created: u64,
    created_wall: i64,
    ended_at: Option<u64>,
    /// The board chosen (none: the setup's own).
    map: Option<MapChoice>,
    /// A seat's name to show instead of its account name (the computer
    /// with a computer opponent's deck).
    titles: [Option<String>; 2],
    /// Listed for spectators (not a private room's match).
    listed: bool,
    /// Spectators' connections.
    spectators: Vec<ConnId>,
}

impl Hosted {
    /// The players' names as the screens show them.
    fn shown_names(&self) -> [String; 2] {
        [0, 1].map(|s| self.titles[s].clone().unwrap_or_else(|| self.players[s].clone()))
    }
}

enum Job {
    Hash { conn: ConnId, username: String, invite: String, password: String, cost: PasswordCost },
    Verify { conn: ConnId, username: String, user: Option<User>, password: String, phc: String },
    /// Check the old password, then hash the new one.
    Change { conn: ConnId, user_id: i64, old: String, new: String, phc: String, cost: PasswordCost },
}

enum Done {
    Hashed { conn: ConnId, username: String, invite: String, phc: String },
    Verified { conn: ConnId, username: String, user: Option<User>, ok: bool },
    /// `None`: the old password was wrong.
    Changed { conn: ConnId, user_id: i64, phc: Option<String> },
}

/// Failed sign-ins in a sliding window: per address, per (name, address)
/// and per name over all addresses (see [`ATTEMPTS_PER_NAME`]).
#[derive(Default)]
pub struct Attempts {
    by_address: HashMap<IpAddr, VecDeque<u64>>,
    by_name_and_address: HashMap<(String, Option<IpAddr>), VecDeque<u64>>,
    by_name: HashMap<String, VecDeque<u64>>,
}

impl Attempts {
    fn trim(q: &mut VecDeque<u64>, now: u64) {
        while q.front().is_some_and(|&t| now.saturating_sub(t) >= ATTEMPT_WINDOW_MS) {
            q.pop_front();
        }
    }

    fn full<K: std::hash::Hash + Eq>(map: &mut HashMap<K, VecDeque<u64>>, key: &K, limit: usize, now: u64) -> bool {
        map.get_mut(key).is_some_and(|q| {
            Self::trim(q, now);
            q.len() >= limit
        })
    }

    /// Whether another attempt is allowed.
    pub fn allowed(&mut self, peer: Option<IpAddr>, name: Option<&str>, now: u64) -> bool {
        if let Some(ip) = peer {
            if Self::full(&mut self.by_address, &ip, ATTEMPTS_PER_ADDRESS, now) {
                return false;
            }
        }
        if let Some(name) = name {
            let key = username_key(name);
            if Self::full(&mut self.by_name_and_address, &(key.clone(), peer), ATTEMPTS_PER_NAME, now)
                || Self::full(&mut self.by_name, &key, ATTEMPTS_PER_NAME_ALL_ADDRESSES, now)
            {
                return false;
            }
        }
        true
    }

    pub fn failed(&mut self, peer: Option<IpAddr>, name: Option<&str>, now: u64) {
        if let Some(ip) = peer {
            self.by_address.entry(ip).or_default().push_back(now);
        }
        if let Some(name) = name {
            let key = username_key(name);
            self.by_name_and_address.entry((key.clone(), peer)).or_default().push_back(now);
            self.by_name.entry(key).or_default().push_back(now);
        }
        // Forget entries whose window is over (bounded memory).
        if self.by_address.len() + self.by_name_and_address.len() + self.by_name.len() > 3 * 4096 {
            self.by_address.retain(|_, q| {
                Self::trim(q, now);
                !q.is_empty()
            });
            self.by_name_and_address.retain(|_, q| {
                Self::trim(q, now);
                !q.is_empty()
            });
            self.by_name.retain(|_, q| {
                Self::trim(q, now);
                !q.is_empty()
            });
        }
    }
}

/// Open sign-ups per address and in total in a sliding window.
#[derive(Default)]
pub struct SignUps {
    by_address: HashMap<IpAddr, VecDeque<u64>>,
    all: VecDeque<u64>,
}

impl SignUps {
    fn trim(q: &mut VecDeque<u64>, now: u64) {
        while q.front().is_some_and(|&t| now.saturating_sub(t) >= SIGNUP_WINDOW_MS) {
            q.pop_front();
        }
    }

    /// Whether another open sign-up from `peer` is allowed now.
    pub fn allowed(&mut self, peer: Option<IpAddr>, now: u64) -> bool {
        Self::trim(&mut self.all, now);
        if self.all.len() >= SIGNUPS_TOTAL {
            return false;
        }
        match peer.and_then(|ip| self.by_address.get_mut(&ip)) {
            Some(q) => {
                Self::trim(q, now);
                q.len() < SIGNUPS_PER_ADDRESS
            }
            None => true,
        }
    }

    /// Whether the server-wide limit (not one address's) is reached.
    pub fn total_full(&mut self, now: u64) -> bool {
        Self::trim(&mut self.all, now);
        self.all.len() >= SIGNUPS_TOTAL
    }

    pub fn record(&mut self, peer: Option<IpAddr>, now: u64) {
        self.all.push_back(now);
        if let Some(ip) = peer {
            self.by_address.entry(ip).or_default().push_back(now);
        }
        // Forget addresses whose window is over (bounded memory).
        if self.by_address.len() > 4096 {
            self.by_address.retain(|_, q| {
                Self::trim(q, now);
                !q.is_empty()
            });
        }
    }
}

pub struct Hub {
    settings: HubSettings,
    data: HubData,
    compat: Compat,
    accounts: Accounts,
    lobby: Lobby,
    conns: BTreeMap<ConnId, Conn>,
    matches: BTreeMap<String, Hosted>,
    /// Ticket → (match id, seat).
    tickets: HashMap<String, (String, u8)>,
    /// MODES: N-player matches, their tickets, and the lobby connections
    /// whose client said `modes`.
    multis: BTreeMap<String, HostedMulti>,
    multi_tickets: HashMap<String, (String, u8)>,
    modes_conns: std::collections::HashSet<ConnId>,
    /// Watch ticket → match id.
    watch_tickets: HashMap<String, String>,
    /// Players who turned "Allow spectators" off (name keys).
    no_spectators: std::collections::HashSet<String>,
    next_conn: ConnId,
    jobs: mpsc::Sender<Job>,
    done: mpsc::Receiver<Done>,
    in_flight: usize,
    dummy_hash: String,
    attempts: Attempts,
    signups: SignUps,
    /// Connections of the computer opponent's threads.
    internal_tx: mpsc::Sender<HubConn>,
    internal_rx: mpsc::Receiver<HubConn>,
    log: Box<dyn FnMut(&str) + Send>,
    /// Panics caught so far (they end a match or a connection, not the hub).
    contained: u64,
    last_prune: u64,
    /// Who is online changed since the last list went out.
    players_dirty: bool,
    players_sent_at: Option<u64>,
    /// Email addresses and password reset (none: this server sends no
    /// email; see [`Hub::with_email`]).
    email: Option<email_hub::EmailHub>,
}

impl Hub {
    pub fn new(settings: HubSettings, data: HubData, accounts: Accounts, log: impl FnMut(&str) + Send + 'static) -> Self {
        let compat = Compat::current(data_fingerprint(&data.cards, &data.tables));
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done) = mpsc::channel::<Done>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        for _ in 0..settings.auth_workers.max(1) {
            let (rx, tx) = (job_rx.clone(), done_tx.clone());
            std::thread::spawn(move || loop {
                let job = match rx.lock().map(|r| r.recv()) {
                    Ok(Ok(job)) => job,
                    _ => return,
                };
                let done = match job {
                    Job::Hash { conn, username, invite, password, cost } => {
                        Done::Hashed { conn, username, invite, phc: accounts::hash_password(&password, cost) }
                    }
                    Job::Verify { conn, username, user, password, phc } => {
                        let ok = accounts::verify_password(&password, &phc) && user.is_some();
                        Done::Verified { conn, username, user, ok }
                    }
                    Job::Change { conn, user_id, old, new, phc, cost } => {
                        let phc = accounts::verify_password(&old, &phc).then(|| accounts::hash_password(&new, cost));
                        Done::Changed { conn, user_id, phc }
                    }
                };
                if tx.send(done).is_err() {
                    return;
                }
            });
        }
        let (internal_tx, internal_rx) = mpsc::channel();
        let dummy_hash = accounts::dummy_hash(settings.password_cost);
        Self {
            lobby: Lobby::new(data.computer_deck.is_some()),
            settings,
            data,
            compat,
            accounts,
            conns: BTreeMap::new(),
            matches: BTreeMap::new(),
            tickets: HashMap::new(),
            multis: BTreeMap::new(),
            multi_tickets: HashMap::new(),
            modes_conns: Default::default(),
            watch_tickets: HashMap::new(),
            no_spectators: Default::default(),
            next_conn: 1,
            jobs,
            done,
            in_flight: 0,
            dummy_hash,
            attempts: Attempts::default(),
            signups: SignUps::default(),
            internal_tx,
            internal_rx,
            log: Box::new(log),
            contained: 0,
            last_prune: 0,
            players_dirty: false,
            players_sent_at: None,
            email: None,
        }
    }

    pub fn compat(&self) -> &Compat {
        &self.compat
    }
    pub fn accounts(&self) -> &Accounts {
        &self.accounts
    }
    /// Matches being hosted (running, waiting or just ended).
    pub fn match_count(&self) -> usize {
        self.matches.len() + self.multis.len()
    }
    pub fn connection_count(&self) -> usize {
        self.conns.len()
    }

    fn log(&mut self, line: &str) {
        (self.log)(line);
    }

    /// Serve until `stop` is set.
    pub fn run(&mut self, acceptors: &mut [Box<dyn HubAcceptor + Send>], stop: &AtomicBool) -> io::Result<()> {
        let start = Instant::now();
        let mut failed_passes = 0;
        while !stop.load(Ordering::Relaxed) {
            let now = start.elapsed().as_millis() as u64;
            // The last net: `step` contains a connection's or a match's
            // panic itself; one anywhere else costs this pass, not the
            // server. A hub that panics on every pass stops, so the service
            // restarts it instead of it spinning.
            match catch_unwind(AssertUnwindSafe(|| self.step(acceptors, now))) {
                Ok(busy) => {
                    failed_passes = 0;
                    if !busy? {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Err(panic) => {
                    self.contained += 1;
                    self.log(&format!("a hub pass panicked: {}", panic_text(&*panic)));
                    failed_passes += 1;
                    if failed_passes >= MAX_FAILED_PASSES {
                        return Err(io::Error::other("the hub panicked on every pass"));
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        for (_, mut c) in std::mem::take(&mut self.conns) {
            c.conn.close();
        }
        Ok(())
    }

    /// One pass: accept, finish auth jobs, read every connection, tick the
    /// matches and the lobby. Returns whether anything happened.
    pub fn step(&mut self, acceptors: &mut [Box<dyn HubAcceptor + Send>], now: u64) -> io::Result<bool> {
        let mut busy = false;
        let mut pending = self.conns.values().filter(|c| matches!(c.role, Role::Pending)).count();
        for acceptor in acceptors.iter_mut() {
            // Beyond MAX_PENDING connections in their handshake, new ones
            // wait in the listen backlog (accepted on a later pass) rather
            // than being turned away: a refusal closed on an unread Hello
            // is often lost to a reset before the client can read it.
            while pending < MAX_PENDING {
                match acceptor.accept() {
                    Ok(Some((conn, peer))) => {
                        busy = true;
                        pending += 1;
                        self.admit(conn, peer, false, now);
                    }
                    Ok(None) => break,
                    // One failed accept (a client gone mid-handshake, out
                    // of descriptors) must not stop the server.
                    Err(e) => {
                        self.log(&format!("accepting a connection failed: {e}"));
                        break;
                    }
                }
            }
            for note in acceptor.take_notes() {
                self.log(&note);
            }
        }
        while let Ok(conn) = self.internal_rx.try_recv() {
            busy = true;
            self.admit(conn, None, true, now);
        }
        while let Ok(done) = self.done.try_recv() {
            busy = true;
            self.in_flight = self.in_flight.saturating_sub(1);
            self.finish_auth(done, now);
        }
        busy |= self.email_step(now);
        let ids: Vec<ConnId> = self.conns.keys().copied().collect();
        for id in ids {
            let seated = self.seat_match(id);
            match catch_unwind(AssertUnwindSafe(|| self.read_conn(id, now))) {
                Ok(read) => busy |= read,
                Err(panic) => {
                    busy = true;
                    let mid = self.seat_match(id).or(seated);
                    self.contain_conn(id, mid, &*panic, now);
                }
            }
        }
        // Handshake and signed-out timeouts.
        let stale: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| match &c.role {
                Role::Pending => now.saturating_sub(c.opened) > HANDSHAKE_MS,
                Role::Lobby { user: None, authing: false, .. } => now.saturating_sub(c.opened) > SIGNED_OUT_MS,
                _ => false,
            })
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            self.drop_conn(id, now);
        }
        busy |= self.tick_matches(now);
        busy |= self.tick_multis(now);
        let actions = self.lobby.tick(now);
        self.act(actions, now);
        self.send_players(now);
        if now.saturating_sub(self.last_prune) > 3_600_000 {
            self.last_prune = now;
            match self.accounts.prune_sessions() {
                Ok(n) if n > 0 => self.log(&format!("pruned {n} expired sessions")),
                Ok(_) => {}
                Err(e) => self.log(&format!("pruning sessions failed: {e}")),
            }
            if let (Some(dir), Some(days)) = (self.settings.replay_dir.clone(), self.settings.replay_keep_days) {
                match crate::replay::prune_replays(&dir, Duration::from_secs(days * 86_400)) {
                    Ok(n) if n > 0 => self.log(&format!("deleted {n} replays older than {days} days")),
                    Ok(_) => {}
                    Err(e) => self.log(&format!("pruning replays failed: {e}")),
                }
            }
        }
        Ok(busy)
    }

    /// The messages waiting on one connection, up to this pass's share.
    fn read_conn(&mut self, id: ConnId, now: u64) -> bool {
        let mut busy = false;
        // A few messages per connection per pass keeps it fair.
        for _ in 0..32 {
            let Some(c) = self.conns.get_mut(&id) else { break };
            match c.conn.poll() {
                Received::Empty => break,
                Received::Message(m) => {
                    busy = true;
                    if !matches!(c.role, Role::Pending) && !c.budget.take(now) {
                        let _ = c.conn.send(&ServerFrame::Match(ServerMessage::ProtocolError {
                            message: "message rate limit exceeded".into(),
                        }));
                        self.log(&format!("connection {id} exceeded the message rate; closing it"));
                        self.drop_conn(id, now);
                        break;
                    }
                    self.receive(id, m, now);
                }
                Received::Malformed(message) => {
                    busy = true;
                    let _ = c.conn.send(&ServerFrame::Match(ServerMessage::ProtocolError { message }));
                    self.drop_conn(id, now);
                    break;
                }
                Received::Closed => {
                    busy = true;
                    self.drop_conn(id, now);
                    break;
                }
            }
        }
        busy
    }

    /// The match whose seat connection `id` holds.
    fn seat_match(&self, id: ConnId) -> Option<String> {
        match &self.conns.get(&id)?.role {
            Role::Seat { match_id, .. } | Role::MultiSeat { match_id, .. } => Some(match_id.clone()),
            _ => None,
        }
    }

    /// A panic while reading connection `id`: a seat's match ends with it
    /// (its state can no longer be trusted), anything else loses only the
    /// connection.
    fn contain_conn(&mut self, id: ConnId, mid: Option<String>, panic: &(dyn Any + Send), now: u64) {
        self.contained += 1;
        self.log(&format!("connection {id} panicked: {}", panic_text(panic)));
        if let Some(mid) = mid {
            self.end_broken_match(&mid, now);
        }
        // `drop_conn` takes the connection out of the table first, so a
        // second panic in there cannot bring it back.
        let _ = catch_unwind(AssertUnwindSafe(|| self.drop_conn(id, now)));
    }

    /// End a match after a panic: cancelled for its players, who go back to
    /// the lobby; the broken game is asked nothing but who was in it.
    fn end_broken_match(&mut self, mid: &str, now: u64) {
        self.log(&format!("{mid}: cancelled after a panic"));
        if let Some(ended) = self.matches.get(mid).map(|h| h.ended_at.is_some()) {
            if !ended {
                let _ = catch_unwind(AssertUnwindSafe(|| self.finish(mid, None, now)));
            }
            self.remove_match(mid, now);
        } else if let Some(ended) = self.multis.get(mid).map(|h| h.ended_at.is_some()) {
            if !ended {
                let _ = catch_unwind(AssertUnwindSafe(|| self.finish_multi(mid, None, now)));
            }
            self.remove_multi(mid);
        }
    }

    /// Panics contained so far (a test that must see none).
    #[cfg(test)]
    pub(crate) fn contained(&self) -> u64 {
        self.contained
    }

    fn admit(&mut self, mut conn: HubConn, peer: Option<IpAddr>, internal: bool, now: u64) {
        let pending = self.conns.values().filter(|c| matches!(c.role, Role::Pending)).count();
        if self.conns.len() >= self.settings.max_connections || (!internal && pending >= MAX_PENDING) {
            let _ = conn.send(&ServerFrame::Lobby(LobbyEvent::LobbyRefused { reason: LobbyRefusal::ServerFull }));
            conn.close();
            self.log("refused a connection: the server is full");
            return;
        }
        let cap = self.settings.max_connections_per_address;
        if let Some(ip) = peer.filter(|ip| !internal && cap > 0 && !ip.is_loopback()) {
            if self.conns.values().filter(|c| c.peer == Some(ip)).count() >= cap {
                let _ = conn.send(&ServerFrame::Lobby(LobbyEvent::LobbyRefused { reason: LobbyRefusal::ServerFull }));
                conn.close();
                // The address itself is not logged (the log keeps no addresses).
                self.log("refused a connection: too many connections from one address");
                return;
            }
        }
        let id = self.next_conn;
        self.next_conn += 1;
        let limits = if internal { Limits::UNLIMITED } else { self.settings.limits };
        self.conns.insert(id, Conn { conn, role: Role::Pending, opened: now, budget: Budget::with_limits(now, limits), peer });
    }

    fn send(&mut self, id: ConnId, frame: ServerFrame, now: u64) {
        let Some(c) = self.conns.get_mut(&id) else { return };
        if let Err(e) = c.conn.send(&frame) {
            if e.kind() == io::ErrorKind::TimedOut {
                self.log(&format!("connection {id} stopped reading; closing it"));
            }
            self.drop_conn(id, now);
        }
    }

    /// Open connections' ids, oldest first (tests).
    #[cfg(test)]
    pub(crate) fn conn_ids(&self) -> Vec<ConnId> {
        let mut ids: Vec<ConnId> = self.conns.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Send a lobby event through the normal path (tests).
    #[cfg(test)]
    pub(crate) fn send_lobby_for_test(&mut self, id: ConnId, event: LobbyEvent, now: u64) {
        self.lobby_send(id, event, now);
    }

    fn lobby_send(&mut self, id: ConnId, event: LobbyEvent, now: u64) {
        self.send(id, ServerFrame::Lobby(event), now);
    }

    /// Close and forget a connection, telling its match or the lobby.
    fn drop_conn(&mut self, id: ConnId, now: u64) {
        let Some(mut c) = self.conns.remove(&id) else { return };
        c.conn.close();
        self.modes_conns.remove(&id);
        match c.role {
            Role::MultiSeat { match_id, seat } => {
                if let Some(h) = self.multis.get_mut(&match_id) {
                    if h.seats[usize::from(seat)] == Some(id) {
                        h.seats[usize::from(seat)] = None;
                        let out = h.game.disconnect(seat, now);
                        let line = format!("{match_id}: seat {seat} ({}) disconnected", h.players[usize::from(seat)]);
                        self.log(&line);
                        self.deliver_multi(&match_id, out, now);
                    }
                }
            }
            Role::MultiWatch { match_id } => {
                if let Some(h) = self.multis.get_mut(&match_id) {
                    h.spectators.retain(|&s| s != id);
                    let count = h.spectators.len() as u16;
                    let out = h.game.set_watchers(count);
                    self.deliver_multi(&match_id, out, now);
                }
            }
            Role::Seat { match_id, seat } => {
                if let Some(h) = self.matches.get_mut(&match_id) {
                    if h.seats[usize::from(seat)] == Some(id) {
                        h.seats[usize::from(seat)] = None;
                        let out = h.game.disconnect(seat, now);
                        let line = format!("{match_id}: seat {seat} ({}) disconnected", h.players[usize::from(seat)]);
                        self.log(&line);
                        self.deliver(&match_id, out, now);
                    }
                }
            }
            Role::Lobby { user: Some(u), account: false, .. } => {
                let actions = self.lobby.leave(&u.name);
                self.log(&format!("{} left the lobby", u.name));
                self.act(actions, now);
            }
            Role::Watch { match_id } => {
                if let Some(h) = self.matches.get_mut(&match_id) {
                    h.spectators.retain(|&s| s != id);
                    let count = h.spectators.len() as u16;
                    let out = h.game.set_watchers(count);
                    self.deliver(&match_id, out, now);
                }
            }
            _ => {}
        }
    }

    fn receive(&mut self, id: ConnId, frame: ClientFrame, now: u64) {
        let Some(c) = self.conns.get(&id) else { return };
        #[cfg(test)]
        if let Role::Lobby { user: Some(u), .. } = &c.role {
            test_panic(&u.name);
        }
        match (&c.role, frame) {
            (Role::Pending, ClientFrame::Match(ClientMessage::Hello { compat, join: Join::Watch { ticket }, .. }))
                if self.watch_tickets.get(&ticket).is_some_and(|mid| self.multis.contains_key(mid)) =>
            {
                self.multi_watch_hello(id, compat, ticket, now)
            }
            (Role::Pending, ClientFrame::Match(ClientMessage::Hello { compat, join: Join::Watch { ticket }, .. })) => {
                self.watch_hello(id, compat, ticket, now)
            }
            (Role::Pending, ClientFrame::Match(ClientMessage::Hello { compat, join, deck, presence, spectate }))
                if self.is_multi_join(&join) =>
            {
                self.multi_seat_hello(id, compat, join, deck, presence, spectate, now)
            }
            (Role::Pending, ClientFrame::Match(ClientMessage::Hello { compat, join, deck, presence, spectate })) => {
                self.seat_hello(id, compat, join, deck, presence, spectate, now)
            }
            (Role::Pending, ClientFrame::Lobby(LobbyRequest::LobbyHello { lobby_version, compat, client, account, modes })) => {
                let refusal = if lobby_version != LOBBY_VERSION {
                    Some(LobbyRefusal::LobbyVersion { client: lobby_version, server: LOBBY_VERSION })
                } else if account {
                    // Account-only: no rules data, only the protocol.
                    (compat.protocol != self.compat.protocol).then(|| LobbyRefusal::Incompatible {
                        detail: roses_net::Incompatible::Protocol {
                            client: compat.protocol.to_string(),
                            server: self.compat.protocol.to_string(),
                        },
                    })
                } else {
                    compat.check(&self.compat).err().map(|detail| LobbyRefusal::Incompatible { detail })
                };
                if let Some(reason) = refusal {
                    self.log(&format!("refused a lobby client ({client}): {reason:?}"));
                    self.lobby_send(id, LobbyEvent::LobbyRefused { reason }, now);
                    self.drop_conn(id, now);
                    return;
                }
                if let Some(c) = self.conns.get_mut(&id) {
                    c.role = Role::Lobby { user: None, authing: false, account };
                    c.opened = now;
                }
                if modes && !account {
                    self.modes_conns.insert(id);
                }
                let welcome = LobbyEvent::LobbyWelcome {
                    lobby_version: LOBBY_VERSION,
                    server: self.settings.name.clone(),
                    computer: self.data.computer_deck.is_some(),
                    invite_required: !self.settings.open_signup,
                    choices: true,
                    email: self.email.is_some(),
                    spectate: self.settings.max_spectators > 0 && self.settings.max_spectators_per_match > 0,
                    modes: true,
                };
                self.lobby_send(id, welcome, now);
            }
            (Role::Pending, ClientFrame::Match(_)) => {
                self.send(id, ServerFrame::Match(ServerMessage::Refused { reason: Refusal::ExpectedHello }), now);
                self.drop_conn(id, now);
            }
            (Role::Pending, ClientFrame::Lobby(_)) => {
                self.lobby_send(id, LobbyEvent::LobbyRefused { reason: LobbyRefusal::ExpectedHello }, now);
                self.drop_conn(id, now);
            }
            (Role::Seat { match_id, seat }, ClientFrame::Match(m)) => {
                let (match_id, seat) = (match_id.clone(), *seat);
                if matches!(m, ClientMessage::Hello { .. }) {
                    return;
                }
                let Some(h) = self.matches.get_mut(&match_id) else { return };
                let out = h.game.handle(seat, m, now);
                self.deliver(&match_id, out, now);
            }
            (Role::Seat { .. }, ClientFrame::Lobby(_)) => {}
            (Role::Watch { match_id }, ClientFrame::Match(m)) => {
                let match_id = match_id.clone();
                match m {
                    // A fresh spectator snapshot.
                    ClientMessage::Sync => {
                        let snapshot = self.matches.get(&match_id).and_then(|h| h.game.spectator_snapshot());
                        if let Some(snapshot) = snapshot {
                            self.send(id, ServerFrame::Match(ServerMessage::Snapshot { snapshot }), now);
                        }
                    }
                    // A spectator never acts and never streams presses.
                    ClientMessage::Command { .. } | ClientMessage::Forfeit { .. } => {
                        let message = "spectators cannot act".into();
                        self.send(id, ServerFrame::Match(ServerMessage::ProtocolError { message }), now);
                    }
                    ClientMessage::Hello { .. } | ClientMessage::Presence { .. } => {}
                }
            }
            (Role::Watch { .. }, ClientFrame::Lobby(_)) => {}
            (Role::MultiSeat { match_id, seat }, ClientFrame::Match(m)) => {
                let (match_id, seat) = (match_id.clone(), *seat);
                if matches!(m, ClientMessage::Hello { .. }) {
                    return;
                }
                let Some(h) = self.multis.get_mut(&match_id) else { return };
                let out = h.game.handle(seat, m, now);
                self.deliver_multi(&match_id, out, now);
            }
            (Role::MultiWatch { match_id }, ClientFrame::Match(m)) => {
                let match_id = match_id.clone();
                match m {
                    ClientMessage::Sync => {
                        let snapshot = self.multis.get(&match_id).and_then(|h| h.game.spectator_snapshot());
                        if let Some(snapshot) = snapshot {
                            self.send(id, ServerFrame::Match(ServerMessage::Snapshot { snapshot }), now);
                        }
                    }
                    ClientMessage::Command { .. } | ClientMessage::Forfeit { .. } => {
                        let message = "spectators cannot act".into();
                        self.send(id, ServerFrame::Match(ServerMessage::ProtocolError { message }), now);
                    }
                    ClientMessage::Hello { .. } | ClientMessage::Presence { .. } => {}
                }
            }
            (Role::MultiSeat { .. } | Role::MultiWatch { .. }, ClientFrame::Lobby(_)) => {}
            (Role::Lobby { .. }, ClientFrame::Lobby(r)) => self.lobby_request(id, r, now),
            (Role::Lobby { .. }, ClientFrame::Match(_)) => {}
        }
    }

    // ---- match connections ----------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn seat_hello(
        &mut self,
        id: ConnId,
        compat: Compat,
        join: Join,
        deck: Option<DeckSubmission>,
        presence: bool,
        spectate: bool,
        now: u64,
    ) {
        let found = match &join {
            Join::Ticket { ticket } => self.tickets.get(ticket).cloned(),
            Join::Resume { token } => self
                .matches
                .iter()
                .find_map(|(mid, h)| (0u8..2).find(|&s| h.tokens[usize::from(s)] == *token).map(|s| (mid.clone(), s))),
            Join::Seat { .. } | Join::Watch { .. } => None,
        };
        let Some((match_id, seat)) = found else {
            self.send(id, ServerFrame::Match(ServerMessage::Refused { reason: Refusal::BadToken }), now);
            self.drop_conn(id, now);
            return;
        };
        let h = self.matches.get_mut(&match_id).expect("tickets point at hosted matches");
        // A ticket seats its player the first time and resumes after.
        let join = if h.game.joined(seat) { Join::Resume { token: h.tokens[usize::from(seat)].clone() } } else { Join::Seat { seat: Some(seat) } };
        match h.game.hello_spectating(&compat, &join, deck.as_ref(), presence, spectate, now) {
            Ok((seat, out)) => {
                let old = h.seats[usize::from(seat)].replace(id);
                let player = h.players[usize::from(seat)].clone();
                if let Some(c) = self.conns.get_mut(&id) {
                    c.role = Role::Seat { match_id: match_id.clone(), seat };
                }
                if let Some(old) = old.filter(|&o| o != id) {
                    if let Some(mut c) = self.conns.remove(&old) {
                        c.conn.close();
                    }
                }
                let how = if matches!(join, Join::Resume { .. }) { "rejoined" } else { "joined" };
                self.log(&format!("{match_id}: {player} {how} as seat {seat}"));
                self.deliver(&match_id, out, now);
            }
            Err(reason) => {
                self.log(&format!("{match_id}: refused seat {seat}: {reason:?}"));
                self.send(id, ServerFrame::Match(ServerMessage::Refused { reason }), now);
                self.drop_conn(id, now);
            }
        }
    }

    /// Spectators of all matches.
    fn spectator_count(&self) -> usize {
        self.matches.values().map(|h| h.spectators.len()).sum()
    }

    /// Whether `mid` may be watched now: running, listed, and neither
    /// player turned spectators off.
    fn watchable(&self, h: &Hosted) -> bool {
        h.listed
            && h.game.started()
            && h.game.ended().is_none()
            && h.ended_at.is_none()
            && h.players.iter().all(|p| !self.no_spectators.contains(&username_key(p)))
    }

    /// A match connection with `Join::Watch`: the spectator's welcome and
    /// snapshot, or a refusal.
    fn watch_hello(&mut self, id: ConnId, compat: Compat, ticket: String, now: u64) {
        let refuse = |hub: &mut Self, reason: Refusal| {
            hub.send(id, ServerFrame::Match(ServerMessage::Refused { reason }), now);
            hub.drop_conn(id, now);
        };
        let Some(mid) = self.watch_tickets.get(&ticket).cloned() else { return refuse(self, Refusal::BadToken) };
        if let Err(detail) = compat.check(&self.compat) {
            return refuse(self, Refusal::Incompatible { detail });
        }
        let Some(h) = self.matches.get(&mid) else { return refuse(self, Refusal::NotWatchable) };
        if !self.watchable(h) {
            return refuse(self, Refusal::NotWatchable);
        }
        let total = self.spectator_count() + self.multi_spectator_count();
        if h.spectators.len() >= self.settings.max_spectators_per_match || total >= self.settings.max_spectators {
            return refuse(self, Refusal::WatchersFull);
        }
        let Some(snapshot) = h.game.spectator_snapshot() else { return refuse(self, Refusal::NotWatchable) };
        let watching = ServerMessage::Watching {
            match_id: mid.clone(),
            names: h.shown_names(),
            options: h.game.options(),
            delay_turns: h.game.spectator_delay(),
            multi: None,
        };
        if let Some(c) = self.conns.get_mut(&id) {
            c.role = Role::Watch { match_id: mid.clone() };
        }
        self.send(id, ServerFrame::Match(watching), now);
        self.send(id, ServerFrame::Match(ServerMessage::Snapshot { snapshot }), now);
        if !self.conns.contains_key(&id) {
            return;
        }
        let h = self.matches.get_mut(&mid).expect("checked");
        h.spectators.push(id);
        let count = h.spectators.len() as u16;
        let out = h.game.set_watchers(count);
        // The newcomer already has the snapshot: the count only.
        h.game.take_spectator_messages();
        let note = format!("{mid}: a spectator joined ({count} watching)");
        self.log(&note);
        self.deliver(&mid, out, now);
        for cid in self.matches.get(&mid).map(|h| h.spectators.clone()).unwrap_or_default() {
            self.send(cid, ServerFrame::Match(ServerMessage::Watchers { count }), now);
        }
    }

    /// Send every spectator of `mid` what its match released for them
    /// (serialised once).
    fn fan_out(&mut self, mid: &str, now: u64) {
        let Some(h) = self.matches.get_mut(mid) else { return };
        let messages = h.game.take_spectator_messages();
        if messages.is_empty() || h.spectators.is_empty() {
            return;
        }
        let spectators = h.spectators.clone();
        for m in messages {
            let frame = ServerFrame::Match(m);
            let body = match serde_json::to_vec(&frame) {
                Ok(b) => b,
                Err(e) => return self.log(&format!("{mid}: cannot serialise a spectator frame: {e}")),
            };
            for &cid in &spectators {
                let Some(c) = self.conns.get_mut(&cid) else { continue };
                let sent = match c.conn.send_serialized(&body) {
                    Err(e) if e.kind() == io::ErrorKind::Unsupported => c.conn.send(&frame),
                    r => r,
                };
                if sent.is_err() {
                    self.drop_conn(cid, now);
                }
            }
        }
    }

    /// Send the spectators of `mid` away (a player turned spectators off).
    fn close_spectators(&mut self, mid: &str, now: u64) {
        let Some(h) = self.matches.get_mut(mid) else { return };
        let ids = std::mem::take(&mut h.spectators);
        let out = h.game.set_watchers(0);
        h.game.take_spectator_messages();
        for cid in ids {
            self.send(cid, ServerFrame::Match(ServerMessage::Refused { reason: Refusal::NotWatchable }), now);
            if let Some(mut c) = self.conns.remove(&cid) {
                c.conn.close();
            }
        }
        self.deliver(mid, out, now);
    }

    /// The spectating requests of a signed-in player.
    fn spectate_request(&mut self, id: ConnId, user: &SignedIn, request: LobbyRequest, now: u64) {
        match request {
            LobbyRequest::ListMatches => {
                let mut live: Vec<(&u64, LiveMatch)> = self
                    .matches
                    .iter()
                    .filter(|(_, h)| self.watchable(h))
                    .map(|(mid, h)| {
                        (
                            &h.created,
                            LiveMatch {
                                match_id: mid.clone(),
                                players: h.shown_names(),
                                map: h.map.clone(),
                                round: h.game.spectator_round().unwrap_or(1),
                                watching: h.spectators.len() as u16,
                                seats: Vec::new(), mode: None,
                            },
                        )
                    })
                    .collect();
                if self.modes_conns.contains(&id) {
                    // MODES: N-player matches, for clients that play them.
                    live.extend(self.multis.iter().filter(|(_, h)| self.multi_watchable(h)).map(|(mid, h)| {
                        let names: Vec<String> = h.game.seat_infos().into_iter().map(|s| s.name).collect();
                        (
                            &h.created,
                            LiveMatch {
                                match_id: mid.clone(),
                                players: [names[0].clone(), names[1].clone()],
                                map: h.game.settings().map.clone(),
                                round: h.game.round().unwrap_or(1),
                                watching: h.spectators.len() as u16,
                                seats: names,
                                mode: Some(h.game.settings().mode),
                            },
                        )
                    }));
                }
                live.sort_by(|a, b| b.0.cmp(a.0));
                let matches = live.into_iter().map(|(_, m)| m).collect();
                self.lobby_send(id, LobbyEvent::LiveMatches { matches }, now);
            }
            LobbyRequest::Watch { match_id } if self.multis.contains_key(&match_id) => {
                self.multi_watch_request(id, user, match_id, now)
            }
            LobbyRequest::Watch { match_id } => {
                let me = username_key(&user.name);
                let problem = match self.matches.get(&match_id) {
                    None => Some(LobbyProblem::NoSuchMatch),
                    Some(h) if h.game.ended().is_some() || h.ended_at.is_some() || !h.game.started() => Some(LobbyProblem::NoSuchMatch),
                    Some(h) if !self.watchable(h) || h.players.iter().any(|p| username_key(p) == me) => Some(LobbyProblem::NotWatchable),
                    Some(h)
                        if h.spectators.len() >= self.settings.max_spectators_per_match
                            || self.spectator_count() + self.multi_spectator_count() >= self.settings.max_spectators =>
                    {
                        Some(LobbyProblem::WatchersFull)
                    }
                    Some(_) => None,
                };
                if let Some(problem) = problem {
                    return self.lobby_send(id, LobbyEvent::Problem { problem }, now);
                }
                let h = self.matches.get(&match_id).expect("checked");
                let ticket = crate::secret_token();
                let event = LobbyEvent::WatchReady {
                    match_id: match_id.clone(),
                    ticket: ticket.clone(),
                    players: h.shown_names(),
                    map: h.map.clone(),
                    options: h.game.options(),
                };
                // One ticket per request; they last as long as the match.
                self.watch_tickets.insert(ticket, match_id.clone());
                self.log(&format!("{match_id}: {} may watch", user.name));
                self.lobby_send(id, event, now);
            }
            LobbyRequest::AllowSpectators { allow } => {
                let key = username_key(&user.name);
                if allow {
                    self.no_spectators.remove(&key);
                } else if self.no_spectators.insert(key) {
                    if let Some((mid, _)) = self.live_match_of(&user.name) {
                        self.close_spectators(&mid, now);
                    }
                    if let Some((mid, _)) = self.live_multi_of(&user.name) {
                        self.close_multi_spectators(&mid, now);
                    }
                }
            }
            _ => {}
        }
    }

    fn deliver(&mut self, match_id: &str, out: crate::online::Outbox, now: u64) {
        self.deliver_seats(match_id, out, now);
        self.fan_out(match_id, now);
    }

    fn deliver_seats(&mut self, match_id: &str, out: crate::online::Outbox, now: u64) {
        let mut queue: VecDeque<_> = out.into();
        while let Some((seat, msg)) = queue.pop_front() {
            let Some(h) = self.matches.get_mut(match_id) else { return };
            let Some(cid) = h.seats[usize::from(seat)] else { continue };
            let Some(c) = self.conns.get_mut(&cid) else {
                h.seats[usize::from(seat)] = None;
                queue.extend(h.game.disconnect(seat, now));
                continue;
            };
            if c.conn.send(&ServerFrame::Match(msg)).is_err() {
                let mut c = self.conns.remove(&cid).expect("present");
                c.conn.close();
                h.seats[usize::from(seat)] = None;
                queue.extend(h.game.disconnect(seat, now));
            }
        }
    }

    fn tick_matches(&mut self, now: u64) -> bool {
        let mut busy = false;
        let ids: Vec<String> = self.matches.keys().cloned().collect();
        for mid in ids {
            match catch_unwind(AssertUnwindSafe(|| self.tick_match(&mid, now))) {
                Ok(ticked) => busy |= ticked,
                Err(panic) => {
                    busy = true;
                    self.contained += 1;
                    self.log(&format!("{mid} panicked: {}", panic_text(&*panic)));
                    self.end_broken_match(&mid, now);
                }
            }
        }
        busy
    }

    fn tick_match(&mut self, mid: &str, now: u64) -> bool {
        let mut busy = false;
        #[cfg(test)]
        if let Some(h) = self.matches.get(mid) {
            h.players.iter().for_each(|p| test_panic(p));
        }
        let out = self.matches.get_mut(mid).expect("listed").game.tick(now);
        busy |= !out.is_empty();
        self.deliver(mid, out, now);
        let h = self.matches.get(mid).expect("listed");
        if !h.game.started() && h.game.ended().is_none() && now.saturating_sub(h.created) >= self.settings.start_ms {
            self.log(&format!("{mid}: cancelled, both seats did not join in time"));
            self.finish(mid, None, now);
            self.remove_match(mid, now);
            return true;
        }
        if let Some((outcome, cause)) = h.game.ended() {
            match h.ended_at {
                None => {
                    self.finish(mid, Some((outcome, cause)), now);
                    busy = true;
                }
                Some(at) => {
                    let gone = h.seats.iter().all(Option::is_none);
                    let since = now.saturating_sub(at);
                    if (gone && since >= crate::serve::LINGER.as_millis() as u64) || since >= ENDED_KEEP_MS {
                        self.remove_match(mid, now);
                    }
                }
            }
        }
        busy
    }

    /// A match ended (or was cancelled: `None`): log it, keep its replay,
    /// tell its players' lobby connections, free them in the lobby.
    fn finish(&mut self, mid: &str, end: Option<(roses_core::duel::Outcome, roses_net::Cause)>, now: u64) {
        let Some(h) = self.matches.get_mut(mid) else { return };
        h.ended_at = Some(now);
        let players = h.players.clone();
        if let Some((outcome, cause)) = end {
            let row = MatchRow {
                id: mid.to_string(),
                players: players.clone(),
                started: h.created_wall,
                ended: accounts::now_seconds(),
                outcome: match outcome.winner {
                    Some(w) => format!("{} won ({:?})", players[usize::from(w & 1)], outcome.reason),
                    None => format!("draw ({:?})", outcome.reason),
                },
                cause: format!("{cause:?}"),
                revisions: h.game.revision() as i64,
            };
            let replay = h.game.replay().clone();
            let line = format!("{mid}: ended, {} after {} revisions", row.outcome, row.revisions);
            if let Err(e) = self.accounts.record_match(&row) {
                self.log(&format!("{mid}: could not log the match: {e}"));
            }
            if let Some(dir) = self.settings.replay_dir.clone() {
                let path = dir.join(format!("{mid}.json"));
                let written = std::fs::create_dir_all(&dir)
                    .and_then(|_| std::fs::write(&path, serde_json::to_vec(&replay).unwrap_or_default()));
                if let Err(e) = written {
                    self.log(&format!("{mid}: replay not written: {e}"));
                }
            }
            self.log(&line);
        }
        for seat in 0..2usize {
            let name = &players[seat];
            if name == COMPUTER_NAME {
                continue;
            }
            let result = match end {
                None => MatchResult::Cancelled,
                Some((o, _)) => match o.winner {
                    None => MatchResult::Draw,
                    Some(w) if usize::from(w & 1) == seat => MatchResult::Won,
                    Some(_) => MatchResult::Lost,
                },
            };
            if let Some(cid) = self.lobby.conn_of(name) {
                let event = LobbyEvent::MatchOver { match_id: mid.to_string(), result, cause: end.map(|(_, c)| c) };
                self.lobby_send(cid, event, now);
            }
        }
        let names: Vec<&str> = players.iter().filter(|n| *n != COMPUTER_NAME).map(String::as_str).collect();
        let actions = self.lobby.ended(&names);
        self.act(actions, now);
    }

    fn remove_match(&mut self, mid: &str, now: u64) {
        let Some(h) = self.matches.remove(mid) else { return };
        for t in &h.tickets {
            self.tickets.remove(t);
        }
        self.watch_tickets.retain(|_, m| m != mid);
        for cid in h.seats.into_iter().flatten().chain(h.spectators) {
            if let Some(mut c) = self.conns.remove(&cid) {
                c.conn.close();
            }
        }
        let _ = now;
    }

    /// The running (not ended) match of `name`, with its seat.
    fn live_match_of(&self, name: &str) -> Option<(String, u8)> {
        let key = username_key(name);
        self.matches.iter().find_map(|(mid, h)| {
            if h.ended_at.is_some() {
                return None;
            }
            (0u8..2).find(|&s| username_key(&h.players[usize::from(s)]) == key).map(|s| (mid.clone(), s))
        })
    }

    fn match_ready(&self, mid: &str, seat: u8) -> Option<LobbyEvent> {
        let h = self.matches.get(mid)?;
        Some(LobbyEvent::MatchReady {
            match_id: mid.to_string(),
            seat,
            ticket: h.tickets[usize::from(seat)].clone(),
            opponent: h.players[usize::from(1 - seat)].clone(),
            options: h.game.options(),
            map: h.map.clone(),
            opponent_title: h.titles[usize::from(1 - seat)].clone(),
        })
    }

    /// A lobby request's match choices as the server can play them: a
    /// known board or a 7 × 7 map of valid terrain codes, a computer deck
    /// that keeps the deck rules (leader and leader rank included).
    fn check_choices(&self, request: &LobbyRequest) -> Result<(), LobbyProblem> {
        let (map, computer) = match request {
            LobbyRequest::Challenge { map, .. } | LobbyRequest::CreateRoom { map } => (map, &None),
            LobbyRequest::PlayComputer { map, computer } => (map, computer),
            // MODES: the room settings.
            LobbyRequest::CreateModeRoom { settings } | LobbyRequest::ModeRoomSettings { settings } => {
                return settings.check(self.data.maps.len());
            }
            _ => return Ok(()),
        };
        if let Some(m) = map {
            m.check(self.data.maps.len())?;
        }
        if let Some(c) = computer {
            c.check_name()?;
            if let roses_net::lobby::ComputerDeck::Deck { deck, .. } = c {
                roses_net::deck::check_deck(deck, &self.data.cards).map_err(|_| LobbyProblem::BadComputerDeck)?;
            }
        }
        Ok(())
    }

    /// The board a choice comes to (RANDOM resolved): its terrain and the
    /// choice the players are told; none: the setup's own board.
    fn resolve_map(&self, map: Option<MapChoice>) -> Option<(Vec<u8>, MapChoice)> {
        let game = |index: u8| self.data.maps.get(usize::from(index)).map(|t| (t.clone(), MapChoice::Game { index }));
        match map? {
            MapChoice::Random if self.data.maps.is_empty() => None,
            MapChoice::Random => game((crate::random_u64() % self.data.maps.len() as u64) as u8),
            MapChoice::Game { index } => game(index),
            MapChoice::Own { name, cells } => Some((cells.clone(), MapChoice::Own { name, cells })),
        }
    }

    /// Start a match between two players (`b` = None: the computer) on
    /// `map` (none: the setup's board), the computer with `computer`'s deck;
    /// `listed`: spectators may find and watch it (not a private room's).
    fn create_match(
        &mut self,
        a: String,
        b: Option<String>,
        map: Option<MapChoice>,
        computer: Option<ComputerDeck>,
        listed: bool,
        now: u64,
    ) {
        let humans: Vec<String> = [Some(a.clone()), b.clone()].into_iter().flatten().collect();
        if self.match_count() >= self.settings.max_matches {
            for name in &humans {
                if let Some(cid) = self.lobby.conn_of(name) {
                    self.lobby_send(cid, LobbyEvent::Problem { problem: LobbyProblem::ServerBusy }, now);
                }
            }
            return;
        }
        let mut players = [a, b.clone().unwrap_or_else(|| COMPUTER_NAME.to_string())];
        if self.settings.fixed_seed.is_none() && crate::random_u64() & 1 == 1 {
            players.swap(0, 1);
        }
        let mid = format!("match-{:016x}", crate::random_u64());
        let tokens = [crate::secret_token(), crate::secret_token()];
        let tickets = [crate::secret_token(), crate::secret_token()];
        let mut setup = (self.data.setup)(self.settings.fixed_seed.unwrap_or_else(crate::random_u64));
        let map = self.resolve_map(map);
        if let Some((terrain, _)) = &map {
            setup.terrain = terrain.clone();
        }
        let map = map.map(|m| m.1);
        // The computer's deck: the player's choice, a random computer
        // opponent's, else the server's own.
        let computer = match computer {
            Some(ComputerDeck::Deck { name, deck }) => Some((Some(name), deck)),
            Some(ComputerDeck::Random) if !self.data.computer_decks.is_empty() => {
                let i = (crate::random_u64() % self.data.computer_decks.len() as u64) as usize;
                let (name, deck) = self.data.computer_decks[i].clone();
                Some((Some(name), deck))
            }
            _ => self.data.computer_deck.clone().map(|d| (None, d)),
        };
        let titles = players.clone().map(|p| if p == COMPUTER_NAME { computer.as_ref().and_then(|c| c.0.clone()) } else { None });
        let game = OnlineMatch::with_submitted_decks(
            mid.clone(),
            setup,
            self.data.cards.clone(),
            self.settings.policy.clone(),
            tokens.clone(),
        )
        .and_then(|g| g.with_rules(self.data.tables.clone(), self.settings.options))
        .map(|g| g.with_spectator_delay(self.settings.spectate_delay_turns));
        let game = match game {
            Ok(g) => g,
            Err(e) => {
                self.log(&format!("could not create a match: {e}"));
                for name in &humans {
                    if let Some(cid) = self.lobby.conn_of(name) {
                        self.lobby_send(cid, LobbyEvent::Problem { problem: LobbyProblem::ServerBusy }, now);
                    }
                }
                return;
            }
        };
        for (s, t) in tickets.iter().enumerate() {
            self.tickets.insert(t.clone(), (mid.clone(), s as u8));
        }
        self.log(&format!(
            "{mid}: {} (seat 0) vs {} (seat 1){}",
            players[0],
            players[1],
            map.as_ref().map(|m| format!(", board {m:?}")).unwrap_or_default()
        ));
        self.matches.insert(
            mid.clone(),
            Hosted {
                game,
                players: players.clone(),
                tokens,
                tickets: tickets.clone(),
                seats: [None, None],
                created: now,
                created_wall: accounts::now_seconds(),
                ended_at: None,
                map,
                titles,
                listed,
                spectators: Vec::new(),
            },
        );
        let names: Vec<&str> = humans.iter().map(String::as_str).collect();
        let actions = self.lobby.started(&names);
        self.act(actions, now);
        for seat in 0u8..2 {
            let name = players[usize::from(seat)].clone();
            if name == COMPUTER_NAME {
                let deck = computer.as_ref().map(|c| c.1.clone());
                self.spawn_computer(&mid, seat, tickets[usize::from(seat)].clone(), deck);
            } else if let (Some(cid), Some(ready)) = (self.lobby.conn_of(&name), self.match_ready(&mid, seat)) {
                self.lobby_send(cid, ready, now);
            }
        }
    }

    /// The computer opponent: the server's own computer-driven client on
    /// an in-memory connection, with the configured deck.
    fn spawn_computer(&mut self, mid: &str, seat: u8, ticket: String, deck: Option<DeckSubmission>) {
        let Some(deck) = deck else { return };
        let (cards, compat, tx) = (self.data.cards.clone(), self.compat.clone(), self.internal_tx.clone());
        let mid = mid.to_string();
        std::thread::spawn(move || {
            let connect = || {
                let (client, server) = roses_net::transport::memory_pair::<ClientMessage, ServerMessage>();
                tx.send(Box::new(server.retype::<ServerFrame, ClientFrame>()) as HubConn)
                    .map_err(|_| io::Error::other("the server stopped"))?;
                Ok(client)
            };
            let options = crate::play::PlayOptions {
                join: Join::Ticket { ticket },
                deck: Some(deck),
                timeout: Duration::from_secs(4 * 3600),
                // One thread per computer match: polling every 15 ms (not
                // 1) keeps 100 of them cheap; it answers within a frame.
                idle: Duration::from_millis(15),
                ..crate::play::PlayOptions::default()
            };
            if let Err(e) = crate::play::play_bot(connect, &compat, &cards, options, |_| {}) {
                eprintln!("{mid}: the computer player stopped: {e}");
            }
        });
        let _ = seat;
    }

    // ---- lobby connections ------------------------------------------------

    fn signed_in(&self, id: ConnId) -> Option<SignedIn> {
        match &self.conns.get(&id)?.role {
            Role::Lobby { user, .. } => user.clone(),
            _ => None,
        }
    }

    fn set_authing(&mut self, id: ConnId, on: bool) {
        if let Some(Conn { role: Role::Lobby { authing, .. }, .. }) = self.conns.get_mut(&id) {
            *authing = on;
        }
    }

    fn account_conn(&self, id: ConnId) -> bool {
        matches!(self.conns.get(&id), Some(Conn { role: Role::Lobby { account: true, .. }, .. }))
    }

    fn authing(&self, id: ConnId) -> bool {
        matches!(self.conns.get(&id), Some(Conn { role: Role::Lobby { authing: true, .. }, .. }))
    }

    /// A name for a new account: the protocol's rules and reserved words
    /// (`check_new_username`), then this server's [`NameFilter`].
    fn check_new_name(&self, name: &str) -> Result<(), AuthProblem> {
        check_new_username(name)?;
        if self.settings.name_filter.allows(name) {
            Ok(())
        } else {
            Err(AuthProblem::ReservedName)
        }
    }

    fn auth_failed(&mut self, id: ConnId, problem: AuthProblem, now: u64) {
        self.lobby_send(id, LobbyEvent::AuthFailed { problem }, now);
    }

    fn lobby_request(&mut self, id: ConnId, request: LobbyRequest, now: u64) {
        let peer = self.conns.get(&id).and_then(|c| c.peer);
        let user = self.signed_in(id);
        match request {
            LobbyRequest::LobbyHello { .. } => {}
            LobbyRequest::SignUp { username, password, invite } => {
                if user.is_some() || self.authing(id) {
                    return self.auth_failed(id, AuthProblem::Busy, now);
                }
                if let Err(p) = check_username(&username).and_then(|_| check_password(&password)) {
                    return self.auth_failed(id, p, now);
                }
                if !self.attempts.allowed(peer, None, now) {
                    return self.auth_failed(id, AuthProblem::TooManyAttempts, now);
                }
                if invite.trim().is_empty() {
                    // Open sign-up: no invite, a new name only.
                    if !self.settings.open_signup {
                        return self.auth_failed(id, AuthProblem::BadInvite, now);
                    }
                    if let Err(p) = self.check_new_name(&username) {
                        return self.auth_failed(id, p, now);
                    }
                    match self.accounts.user(&username) {
                        Ok(None) => {}
                        // Counted: "taken" tells which names exist, so
                        // asking for many is limited like guessing them.
                        Ok(Some(_)) => {
                            self.attempts.failed(peer, None, now);
                            return self.auth_failed(id, AuthProblem::UsernameTaken, now);
                        }
                        Err(e) => {
                            self.log(&format!("account database error: {e}"));
                            return self.auth_failed(id, AuthProblem::Busy, now);
                        }
                    }
                    if !self.signups.allowed(peer, now) {
                        // The address itself is not logged (the log keeps no addresses).
                        let total = self.signups.total_full(now);
                        self.log(&format!("open sign-up limit reached ({})", if total { "server-wide" } else { "one address" }));
                        return self.auth_failed(id, AuthProblem::SignUpLimit, now);
                    }
                    self.signups.record(peer, now);
                    let cost = self.settings.password_cost;
                    return self.submit(id, Job::Hash { conn: id, username, invite: String::new(), password, cost }, now);
                }
                let problem = match (self.accounts.usable_invite(&invite, &username), self.accounts.user(&username)) {
                    (Ok(None), _) => Some(AuthProblem::BadInvite),
                    (Ok(Some(inv)), Ok(existing)) => match (&inv.for_user, existing) {
                        (None, Some(_)) => Some(AuthProblem::UsernameTaken),
                        (Some(_), None) => Some(AuthProblem::BadInvite),
                        (None, None) => self.check_new_name(&username).err(),
                        _ => None,
                    },
                    (Err(e), _) | (_, Err(e)) => {
                        self.log(&format!("account database error: {e}"));
                        Some(AuthProblem::Busy)
                    }
                };
                if let Some(p) = problem {
                    if p == AuthProblem::BadInvite {
                        self.attempts.failed(peer, None, now);
                    }
                    return self.auth_failed(id, p, now);
                }
                let cost = self.settings.password_cost;
                self.submit(id, Job::Hash { conn: id, username, invite, password, cost }, now);
            }
            LobbyRequest::LogIn { username, password } => {
                if user.is_some() || self.authing(id) {
                    return self.auth_failed(id, AuthProblem::Busy, now);
                }
                if !self.attempts.allowed(peer, Some(&username), now) {
                    return self.auth_failed(id, AuthProblem::TooManyAttempts, now);
                }
                // No account has such a name or password (both rules hold
                // since the first sign-up): no Argon2 job, and the text is
                // not kept in the attempts' maps.
                if check_username(&username).is_err() || password.len() > PASSWORD_MAX {
                    self.attempts.failed(peer, None, now);
                    return self.auth_failed(id, AuthProblem::BadCredentials, now);
                }
                let found = self.accounts.user(&username).unwrap_or_else(|e| {
                    (self.log)(&format!("account database error: {e}"));
                    None
                });
                // An unknown name costs one Argon2 verification too.
                let phc = found.as_ref().map_or_else(|| self.dummy_hash.clone(), |u| u.phc.clone());
                self.submit(id, Job::Verify { conn: id, username, user: found, password, phc }, now);
            }
            LobbyRequest::ResumeSession { session } => {
                if user.is_some() {
                    return;
                }
                if !self.attempts.allowed(peer, None, now) {
                    return self.auth_failed(id, AuthProblem::TooManyAttempts, now);
                }
                match self.accounts.session_user(&session) {
                    Ok(Ok(u)) => self.sign_in(id, u, session, now),
                    Ok(Err(p)) => {
                        self.attempts.failed(peer, None, now);
                        self.auth_failed(id, p, now);
                    }
                    Err(e) => {
                        self.log(&format!("account database error: {e}"));
                        self.auth_failed(id, AuthProblem::Busy, now);
                    }
                }
            }
            LobbyRequest::ChangePassword { old_password, new_password } => {
                let Some(u) = user else { return self.auth_failed(id, AuthProblem::NotSignedIn, now) };
                if self.authing(id) {
                    return self.auth_failed(id, AuthProblem::Busy, now);
                }
                if let Err(p) = check_password(&new_password) {
                    return self.auth_failed(id, p, now);
                }
                if !self.attempts.allowed(peer, Some(&u.name), now) {
                    return self.auth_failed(id, AuthProblem::TooManyAttempts, now);
                }
                let phc = match self.accounts.user_by_id(u.id) {
                    Ok(Some(row)) => row.phc,
                    other => {
                        if let Err(e) = other {
                            self.log(&format!("account database error: {e}"));
                        }
                        return self.auth_failed(id, AuthProblem::Busy, now);
                    }
                };
                let cost = self.settings.password_cost;
                self.submit(id, Job::Change { conn: id, user_id: u.id, old: old_password, new: new_password, phc, cost }, now);
            }
            LobbyRequest::LogOut => {
                if let Some(u) = user {
                    if let Err(e) = self.accounts.end_session(&u.session) {
                        self.log(&format!("account database error: {e}"));
                    }
                    if let Some(Conn { role: Role::Lobby { user, .. }, opened, .. }) = self.conns.get_mut(&id) {
                        *user = None;
                        *opened = now;
                    }
                    let actions = if self.account_conn(id) { Vec::new() } else { self.lobby.leave(&u.name) };
                    self.log(&format!("{} logged out", u.name));
                    self.lobby_send(id, LobbyEvent::SignedOut { elsewhere: false }, now);
                    self.act(actions, now);
                }
            }
            // Keeps idle connections (and their NAT mappings) alive.
            LobbyRequest::Ping => self.lobby_send(id, LobbyEvent::Pong, now),
            r @ (LobbyRequest::AccountStatus
            | LobbyRequest::SetEmail { .. }
            | LobbyRequest::VerifyEmail { .. }
            | LobbyRequest::DismissEmailPrompt
            | LobbyRequest::RequestPasswordReset { .. }
            | LobbyRequest::ResetPassword { .. }) => self.email_request(id, user, r, now),
            // An account-only connection has no lobby.
            _ if self.account_conn(id) => {}
            r @ (LobbyRequest::ListMatches | LobbyRequest::Watch { .. } | LobbyRequest::AllowSpectators { .. }) => match user {
                None => self.auth_failed(id, AuthProblem::NotSignedIn, now),
                Some(u) => self.spectate_request(id, &u, r, now),
            },
            other => match user {
                None => self.auth_failed(id, AuthProblem::NotSignedIn, now),
                Some(u) => {
                    if let Err(problem) = self.check_choices(&other) {
                        self.lobby_send(id, LobbyEvent::Problem { problem }, now);
                        return;
                    }
                    let actions = self.lobby.request(&u.name, &other, now);
                    self.act(actions, now);
                }
            },
        }
    }

    fn submit(&mut self, id: ConnId, job: Job, now: u64) {
        if self.in_flight >= MAX_AUTH_JOBS || self.jobs.send(job).is_err() {
            return self.auth_failed(id, AuthProblem::Busy, now);
        }
        self.in_flight += 1;
        self.set_authing(id, true);
    }

    fn finish_auth(&mut self, done: Done, now: u64) {
        let id = match &done {
            Done::Hashed { conn, .. } | Done::Verified { conn, .. } | Done::Changed { conn, .. } => *conn,
        };
        self.set_authing(id, false);
        if let Done::Changed { user_id, phc, .. } = done {
            let Some(u) = self.signed_in(id).filter(|u| u.id == user_id) else { return };
            let peer = self.conns.get(&id).and_then(|c| c.peer);
            match phc {
                None => {
                    self.attempts.failed(peer, Some(&u.name), now);
                    self.auth_failed(id, AuthProblem::BadCredentials, now);
                }
                Some(phc) => match self.accounts.set_password(user_id, &phc, &u.session) {
                    Ok(()) => {
                        self.log(&format!("{} changed their password", u.name));
                        self.lobby_send(id, LobbyEvent::PasswordChanged, now);
                    }
                    Err(e) => {
                        self.log(&format!("account database error: {e}"));
                        self.auth_failed(id, AuthProblem::Busy, now);
                    }
                },
            }
            return;
        }
        if !self.conns.contains_key(&id) || self.signed_in(id).is_some() {
            return;
        }
        let peer = self.conns.get(&id).and_then(|c| c.peer);
        match done {
            Done::Hashed { username, invite, phc, .. } => {
                let created = if invite.is_empty() {
                    self.accounts.sign_up_open(&username, &phc)
                } else {
                    self.accounts.sign_up(&username, &phc, &invite)
                };
                match created {
                    Ok(Ok(name)) => match self.accounts.user(&name) {
                        Ok(Some(u)) => {
                            self.log(&format!("{name} signed up"));
                            match self.accounts.create_session(u.id) {
                                Ok(session) => self.sign_in(id, u, session, now),
                                Err(e) => {
                                    self.log(&format!("account database error: {e}"));
                                    self.auth_failed(id, AuthProblem::Busy, now);
                                }
                            }
                        }
                        _ => self.auth_failed(id, AuthProblem::Busy, now),
                    },
                    Ok(Err(p)) => {
                        self.attempts.failed(peer, None, now);
                        self.auth_failed(id, p, now);
                    }
                    Err(e) => {
                        self.log(&format!("account database error: {e}"));
                        self.auth_failed(id, AuthProblem::Busy, now);
                    }
                }
            }
            Done::Changed { .. } => {}
            Done::Verified { username, user, ok, .. } => match user {
                Some(u) if ok && u.disabled => self.auth_failed(id, AuthProblem::Disabled, now),
                Some(u) if ok => match self.accounts.create_session(u.id) {
                    Ok(session) => self.sign_in(id, u, session, now),
                    Err(e) => {
                        self.log(&format!("account database error: {e}"));
                        self.auth_failed(id, AuthProblem::Busy, now);
                    }
                },
                _ => {
                    self.attempts.failed(peer, Some(&username), now);
                    self.auth_failed(id, AuthProblem::BadCredentials, now);
                }
            },
        }
    }

    fn sign_in(&mut self, id: ConnId, user: User, session: String, now: u64) {
        if self.account_conn(id) {
            // The launcher's Profile page: no lobby, nobody signed out.
            if let Some(Conn { role: Role::Lobby { user: slot, .. }, .. }) = self.conns.get_mut(&id) {
                *slot = Some(SignedIn { id: user.id, name: user.name.clone(), session: session.clone() });
            }
            self.log(&format!("{} signed in (account page)", user.name));
            return self.lobby_send(id, LobbyEvent::SignedIn { username: user.name, session }, now);
        }
        // A server that requires a verified address keeps others out of the
        // lobby (the session stays: the launcher adds the address with it).
        if !self.email_allows_lobby(&user, now) {
            self.log(&format!("{} has no verified email address; not admitted to the lobby", user.name));
            return self.auth_failed(id, AuthProblem::EmailNotVerified, now);
        }
        // The same account signed in elsewhere in the lobby: that connection
        // is signed out (account-only connections stay).
        let others: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(cid, c)| {
                **cid != id && matches!(&c.role, Role::Lobby { user: Some(u), account: false, .. } if u.id == user.id)
            })
            .map(|(cid, _)| *cid)
            .collect();
        for other in others {
            if let Some(Conn { role: Role::Lobby { user, .. }, opened, .. }) = self.conns.get_mut(&other) {
                *user = None;
                *opened = now;
            }
            self.lobby_send(other, LobbyEvent::SignedOut { elsewhere: true }, now);
        }
        if let Some(Conn { role: Role::Lobby { user: slot, .. }, .. }) = self.conns.get_mut(&id) {
            *slot = Some(SignedIn { id: user.id, name: user.name.clone(), session: session.clone() });
        }
        self.log(&format!("{} signed in", user.name));
        self.lobby_send(id, LobbyEvent::SignedIn { username: user.name.clone(), session }, now);
        let live = self.live_match_of(&user.name);
        let live_multi = self.live_multi_of(&user.name);
        let actions = self.lobby.enter(&user.name, id, live.is_some() || live_multi.is_some());
        self.act(actions, now);
        // Back in the lobby during a match (the game restarted): the
        // ticket rejoins the seat.
        if let Some(ready) = live.and_then(|(mid, seat)| self.match_ready(&mid, seat)) {
            self.lobby_send(id, ready, now);
        }
        if let Some(ready) = live_multi.and_then(|(mid, seat)| self.multi_ready(&mid, seat)) {
            self.lobby_send(id, ready, now);
        }
    }

    fn act(&mut self, actions: Vec<Action>, now: u64) {
        for a in actions {
            match a {
                Action::Send(cid, event) => self.lobby_send(cid, event, now),
                Action::Pair([a, b], map) => self.create_match(a, Some(b), map, None, true, now),
                Action::PairPrivate([a, b], map) => self.create_match(a, Some(b), map, None, false, now),
                Action::PairComputer(a, map, computer) => self.create_match(a, None, map, computer, true, now),
                Action::StartMulti(players, settings) => self.create_multi(players, settings, now),
                Action::Players => self.players_dirty = true,
            }
        }
    }

    /// Send everyone the list of who is online when it changed: at most
    /// one list per pass, serialised once, and spaced out as the lobby
    /// grows (every list goes to every player, so its cost grows with the
    /// square of the players: [`players_interval_ms`]).
    fn send_players(&mut self, now: u64) {
        if !self.players_dirty {
            return;
        }
        let wait = players_interval_ms(self.lobby.online());
        if self.players_sent_at.is_some_and(|at| now.saturating_sub(at) < wait) {
            return;
        }
        self.players_dirty = false;
        self.players_sent_at = Some(now);
        let frame = ServerFrame::Lobby(self.lobby.list());
        let body = match serde_json::to_vec(&frame) {
            Ok(b) => b,
            Err(e) => return self.log(&format!("cannot serialise the player list: {e}")),
        };
        for id in self.lobby.conns() {
            let Some(c) = self.conns.get_mut(&id) else { continue };
            let sent = match c.conn.send_serialized(&body) {
                Err(e) if e.kind() == io::ErrorKind::Unsupported => c.conn.send(&frame),
                r => r,
            };
            if sent.is_err() {
                self.drop_conn(id, now);
            }
        }
    }
}

// ---- MODES: N-player matches (a port addition) ---------------------------

impl Hub {
    /// Whether a match hello is for an N-player match (its ticket or its
    /// resume token).
    fn is_multi_join(&self, join: &Join) -> bool {
        match join {
            Join::Ticket { ticket } => self.multi_tickets.contains_key(ticket),
            Join::Resume { token } => {
                self.multis.values().any(|h| (0..h.game.seat_count()).any(|s| h.game.token(s) == Some(token.as_str())))
            }
            Join::Seat { .. } | Join::Watch { .. } => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn multi_seat_hello(
        &mut self,
        id: ConnId,
        compat: Compat,
        join: Join,
        deck: Option<DeckSubmission>,
        presence: bool,
        spectate: bool,
        now: u64,
    ) {
        let found = match &join {
            Join::Ticket { ticket } => self.multi_tickets.get(ticket).cloned(),
            Join::Resume { token } => self.multis.iter().find_map(|(mid, h)| {
                (0..h.game.seat_count()).find(|&s| h.game.token(s) == Some(token.as_str())).map(|s| (mid.clone(), s))
            }),
            Join::Seat { .. } | Join::Watch { .. } => None,
        };
        let Some((match_id, seat)) = found else {
            self.send(id, ServerFrame::Match(ServerMessage::Refused { reason: Refusal::BadToken }), now);
            self.drop_conn(id, now);
            return;
        };
        let h = self.multis.get_mut(&match_id).expect("tickets point at hosted matches");
        let join = if h.game.joined(seat) {
            Join::Resume { token: h.game.token(seat).unwrap_or_default().to_string() }
        } else {
            Join::Seat { seat: Some(seat) }
        };
        match h.game.hello(&compat, &join, deck.as_ref(), presence, spectate, now) {
            Ok((seat, out)) => {
                let old = h.seats[usize::from(seat)].replace(id);
                let player = h.players[usize::from(seat)].clone();
                // Back from a dropped connection before anything else began:
                // the match has them again.
                if h.released[usize::from(seat)] && !self.lobby.in_match(&player) {
                    h.released[usize::from(seat)] = false;
                    let actions = self.lobby.started(&[player.as_str()]);
                    self.act(actions, now);
                }
                if let Some(c) = self.conns.get_mut(&id) {
                    c.role = Role::MultiSeat { match_id: match_id.clone(), seat };
                }
                if let Some(old) = old.filter(|&o| o != id) {
                    if let Some(mut c) = self.conns.remove(&old) {
                        c.conn.close();
                    }
                }
                let how = if matches!(join, Join::Resume { .. }) { "rejoined" } else { "joined" };
                self.log(&format!("{match_id}: {player} {how} as seat {seat}"));
                self.deliver_multi(&match_id, out, now);
            }
            Err(reason) => {
                self.log(&format!("{match_id}: refused seat {seat}: {reason:?}"));
                self.send(id, ServerFrame::Match(ServerMessage::Refused { reason }), now);
                self.drop_conn(id, now);
            }
        }
    }

    /// Whether an N-player match may be watched now.
    fn multi_watchable(&self, h: &HostedMulti) -> bool {
        h.listed
            && h.game.started()
            && h.game.ended().is_none()
            && h.ended_at.is_none()
            && h.players.iter().all(|p| !self.no_spectators.contains(&username_key(p)))
    }

    fn multi_spectator_count(&self) -> usize {
        self.multis.values().map(|h| h.spectators.len()).sum()
    }

    /// Send the spectators of an N-player match away (a player turned
    /// spectators off).
    fn close_multi_spectators(&mut self, mid: &str, now: u64) {
        let Some(h) = self.multis.get_mut(mid) else { return };
        let ids = std::mem::take(&mut h.spectators);
        let out = h.game.set_watchers(0);
        h.game.take_spectator_messages();
        for cid in ids {
            self.send(cid, ServerFrame::Match(ServerMessage::Refused { reason: Refusal::NotWatchable }), now);
            if let Some(mut c) = self.conns.remove(&cid) {
                c.conn.close();
            }
        }
        self.deliver_multi(mid, out, now);
    }

    fn multi_watch_request(&mut self, id: ConnId, user: &SignedIn, match_id: String, now: u64) {
        let me = username_key(&user.name);
        let total = self.spectator_count() + self.multi_spectator_count();
        let problem = match self.multis.get(&match_id) {
            None => Some(LobbyProblem::NoSuchMatch),
            Some(h) if h.game.ended().is_some() || h.ended_at.is_some() || !h.game.started() => Some(LobbyProblem::NoSuchMatch),
            Some(h) if !self.multi_watchable(h) || h.players.iter().any(|p| username_key(p) == me) => {
                Some(LobbyProblem::NotWatchable)
            }
            Some(h) if h.spectators.len() >= self.settings.max_spectators_per_match || total >= self.settings.max_spectators => {
                Some(LobbyProblem::WatchersFull)
            }
            Some(_) if !self.modes_conns.contains(&id) => Some(LobbyProblem::NotWatchable),
            Some(_) => None,
        };
        if let Some(problem) = problem {
            return self.lobby_send(id, LobbyEvent::Problem { problem }, now);
        }
        let h = self.multis.get(&match_id).expect("checked");
        let names: Vec<String> = h.game.seat_infos().into_iter().map(|s| s.name).collect();
        let ticket = crate::secret_token();
        let event = LobbyEvent::WatchReady {
            match_id: match_id.clone(),
            ticket: ticket.clone(),
            players: [names[0].clone(), names[1].clone()],
            map: h.game.settings().map.clone(),
            options: MatchOptions::default(),
        };
        self.watch_tickets.insert(ticket, match_id.clone());
        self.log(&format!("{match_id}: {} may watch", user.name));
        self.lobby_send(id, event, now);
    }

    fn multi_watch_hello(&mut self, id: ConnId, compat: Compat, ticket: String, now: u64) {
        let refuse = |hub: &mut Self, reason: Refusal| {
            hub.send(id, ServerFrame::Match(ServerMessage::Refused { reason }), now);
            hub.drop_conn(id, now);
        };
        let Some(mid) = self.watch_tickets.get(&ticket).cloned() else { return refuse(self, Refusal::BadToken) };
        if let Err(detail) = compat.check(&self.compat) {
            return refuse(self, Refusal::Incompatible { detail });
        }
        let total = self.spectator_count() + self.multi_spectator_count();
        let Some(h) = self.multis.get(&mid) else { return refuse(self, Refusal::NotWatchable) };
        if !self.multi_watchable(h) {
            return refuse(self, Refusal::NotWatchable);
        }
        if h.spectators.len() >= self.settings.max_spectators_per_match || total >= self.settings.max_spectators {
            return refuse(self, Refusal::WatchersFull);
        }
        let Some(snapshot) = h.game.spectator_snapshot() else { return refuse(self, Refusal::NotWatchable) };
        let names: Vec<String> = h.game.seat_infos().into_iter().map(|s| s.name).collect();
        let watching = ServerMessage::Watching {
            match_id: mid.clone(),
            names: [names[0].clone(), names[1].clone()],
            options: MatchOptions::default(),
            delay_turns: 0,
            multi: Some(h.game.welcome_multi()),
        };
        if let Some(c) = self.conns.get_mut(&id) {
            c.role = Role::MultiWatch { match_id: mid.clone() };
        }
        self.send(id, ServerFrame::Match(watching), now);
        self.send(id, ServerFrame::Match(ServerMessage::Snapshot { snapshot }), now);
        if !self.conns.contains_key(&id) {
            return;
        }
        let h = self.multis.get_mut(&mid).expect("checked");
        h.spectators.push(id);
        let count = h.spectators.len() as u16;
        let out = h.game.set_watchers(count);
        h.game.take_spectator_messages();
        self.log(&format!("{mid}: a spectator joined ({count} watching)"));
        self.deliver_multi(&mid, out, now);
        for cid in self.multis.get(&mid).map(|h| h.spectators.clone()).unwrap_or_default() {
            self.send(cid, ServerFrame::Match(ServerMessage::Watchers { count }), now);
        }
    }

    fn deliver_multi(&mut self, match_id: &str, out: crate::online::Outbox, now: u64) {
        let mut queue: VecDeque<_> = out.into();
        while let Some((seat, msg)) = queue.pop_front() {
            let Some(h) = self.multis.get_mut(match_id) else { return };
            let Some(cid) = h.seats.get(usize::from(seat)).copied().flatten() else { continue };
            let Some(c) = self.conns.get_mut(&cid) else {
                h.seats[usize::from(seat)] = None;
                queue.extend(h.game.disconnect(seat, now));
                continue;
            };
            if c.conn.send(&ServerFrame::Match(msg)).is_err() {
                let mut c = self.conns.remove(&cid).expect("present");
                c.conn.close();
                h.seats[usize::from(seat)] = None;
                queue.extend(h.game.disconnect(seat, now));
            }
        }
        // The spectators' messages, serialised once.
        let Some(h) = self.multis.get_mut(match_id) else { return };
        let messages = h.game.take_spectator_messages();
        if messages.is_empty() || h.spectators.is_empty() {
            return;
        }
        let spectators = h.spectators.clone();
        for m in messages {
            let frame = ServerFrame::Match(m);
            let Ok(body) = serde_json::to_vec(&frame) else { continue };
            for &cid in &spectators {
                let Some(c) = self.conns.get_mut(&cid) else { continue };
                let sent = match c.conn.send_serialized(&body) {
                    Err(e) if e.kind() == io::ErrorKind::Unsupported => c.conn.send(&frame),
                    r => r,
                };
                if sent.is_err() {
                    self.drop_conn(cid, now);
                }
            }
        }
    }

    /// A computer deck and the name shown for it (a random computer
    /// opponent's, else the server's own).
    fn computer_deck_for(&self) -> Option<(String, DeckSubmission)> {
        if !self.data.computer_decks.is_empty() {
            let i = (crate::random_u64() % self.data.computer_decks.len() as u64) as usize;
            return Some(self.data.computer_decks[i].clone());
        }
        self.data.computer_deck.clone().map(|d| (COMPUTER_NAME.to_string(), d))
    }

    /// Start an N-player match from a room: `players` by seat (none: the
    /// server's computer).
    fn create_multi(&mut self, players: Vec<Option<String>>, settings: roses_net::lobby::RoomSettings, now: u64) {
        let humans: Vec<String> = players.iter().flatten().cloned().collect();
        let tell_busy = |hub: &mut Self| {
            for name in &humans {
                if let Some(cid) = hub.lobby.conn_of(name) {
                    hub.lobby_send(cid, LobbyEvent::Problem { problem: LobbyProblem::ServerBusy }, now);
                }
            }
            let names: Vec<&str> = humans.iter().map(String::as_str).collect();
            let actions = hub.lobby.ended(&names);
            hub.act(actions, now);
        };
        if self.matches.len() + self.multis.len() >= self.settings.max_matches {
            return tell_busy(self);
        }
        let mid = format!("multi-{:016x}", crate::random_u64());
        let team = |s: usize| match settings.mode {
            roses_net::lobby::RoomMode::Teams => (s / 2) as u8,
            roses_net::lobby::RoomMode::FreeForAll => s as u8,
        };
        let mut plans = Vec::new();
        let mut names = Vec::new();
        for (s, p) in players.iter().enumerate() {
            let token = crate::secret_token();
            match p {
                Some(name) => {
                    names.push(name.clone());
                    let info = roses_net::SeatInfo { name: name.clone(), computer: false, team: team(s) };
                    plans.push(MultiSeatPlan { info, deck: None, token });
                }
                None => {
                    let Some((shown, deck)) = self.computer_deck_for() else { return tell_busy(self) };
                    names.push(COMPUTER_NAME.to_string());
                    let info = roses_net::SeatInfo { name: shown, computer: true, team: team(s) };
                    plans.push(MultiSeatPlan { info, deck: Some(deck), token });
                }
            }
        }
        let terrain = self.resolve_map(settings.map.clone()).map(|(t, _)| t).unwrap_or_else(|| (self.data.setup)(0).terrain);
        let game = MultiMatch::new(
            mid.clone(),
            plans,
            settings.clone(),
            terrain,
            // Rehearsals (`--match-seed`): an N-player room's match uses the
            // fixed seed too (the shuffle, the first player, the duel LCG);
            // its seats stay as the room had them.
            self.settings.fixed_seed.unwrap_or_else(crate::random_u64),
            self.data.cards.clone(),
            self.data.tables.clone(),
            self.data.computer_tables.clone(),
            self.settings.policy.clone(),
        )
        .map(|g| g.with_computer_delay(self.settings.multi_computer_delay_ms));
        let game = match game {
            Ok(g) => g,
            Err(e) => {
                self.log(&format!("could not create an N-player match: {e}"));
                return tell_busy(self);
            }
        };
        let tickets: Vec<String> = (0..players.len()).map(|_| crate::secret_token()).collect();
        for (s, t) in tickets.iter().enumerate() {
            if players[s].is_some() {
                self.multi_tickets.insert(t.clone(), (mid.clone(), s as u8));
            }
        }
        let shown: Vec<String> = game.seat_infos().into_iter().map(|s| s.name).collect();
        self.log(&format!("{mid}: {:?}, {}", settings.mode, shown.join(" / ")));
        self.multis.insert(
            mid.clone(),
            HostedMulti {
                players: names,
                tickets,
                seats: vec![None; players.len()],
                released: vec![false; players.len()],
                created: now,
                created_wall: accounts::now_seconds(),
                ended_at: None,
                listed: settings.spectators,
                spectators: Vec::new(),
                game,
            },
        );
        let human_names: Vec<&str> = humans.iter().map(String::as_str).collect();
        let actions = self.lobby.started(&human_names);
        self.act(actions, now);
        for (s, p) in players.iter().enumerate() {
            if let Some(name) = p {
                if let (Some(cid), Some(ready)) = (self.lobby.conn_of(name), self.multi_ready(&mid, s as u8)) {
                    self.lobby_send(cid, ready, now);
                }
            }
        }
    }

    fn multi_ready(&self, mid: &str, seat: u8) -> Option<LobbyEvent> {
        let h = self.multis.get(mid)?;
        Some(LobbyEvent::MultiMatchReady {
            match_id: mid.to_string(),
            seat,
            ticket: h.tickets.get(usize::from(seat))?.clone(),
            seats: h.game.seat_infos(),
            settings: h.game.settings().clone(),
        })
    }

    /// The running N-player match of `name`, with its seat.
    fn live_multi_of(&self, name: &str) -> Option<(String, u8)> {
        let key = username_key(name);
        self.multis.iter().find_map(|(mid, h)| {
            if h.ended_at.is_some() {
                return None;
            }
            h.players
                .iter()
                .position(|p| p != COMPUTER_NAME && username_key(p) == key)
                .filter(|&s| !h.released[s])
                .map(|s| (mid.clone(), s as u8))
        })
    }

    fn tick_multis(&mut self, now: u64) -> bool {
        let mut busy = false;
        let ids: Vec<String> = self.multis.keys().cloned().collect();
        for mid in ids {
            match catch_unwind(AssertUnwindSafe(|| self.tick_multi(&mid, now))) {
                Ok(ticked) => busy |= ticked,
                Err(panic) => {
                    busy = true;
                    self.contained += 1;
                    self.log(&format!("{mid} panicked: {}", panic_text(&*panic)));
                    self.end_broken_match(&mid, now);
                }
            }
        }
        busy
    }

    fn tick_multi(&mut self, mid: &str, now: u64) -> bool {
        let mut busy = false;
        #[cfg(test)]
        if let Some(h) = self.multis.get(mid) {
            h.players.iter().for_each(|p| test_panic(p));
        }
        let out = self.multis.get_mut(mid).expect("listed").game.tick(now);
        busy |= !out.is_empty();
        self.deliver_multi(mid, out, now);
        busy |= self.release_seats(mid, now);
        let h = self.multis.get(mid).expect("listed");
        if !h.game.started() && h.game.ended().is_none() && now.saturating_sub(h.created) >= self.settings.start_ms {
            let infos = h.game.seat_infos();
            let joined = (0..h.game.seat_count()).any(|s| h.game.joined(s) && !infos[usize::from(s)].computer);
            if !joined {
                self.log(&format!("{mid}: cancelled, no player joined in time"));
                self.finish_multi(mid, None, now);
                self.remove_multi(mid);
                return true;
            }
            // The players who did not come get the computer.
            let decks: Vec<(String, DeckSubmission)> = (0..4).filter_map(|_| self.computer_deck_for()).collect();
            if decks.is_empty() {
                self.log(&format!("{mid}: cancelled, no computer deck for the missing players"));
                self.finish_multi(mid, None, now);
                self.remove_multi(mid);
                return true;
            }
            let mut decks = decks.into_iter().cycle();
            let h = self.multis.get_mut(mid).expect("listed");
            match h.game.fill_with_computers(|| decks.next().expect("a computer deck")) {
                Ok((filled, out)) => {
                    let missing: Vec<String> =
                        filled.iter().map(|s| std::mem::replace(&mut h.players[usize::from(*s)], COMPUTER_NAME.to_string())).collect();
                    self.log(&format!("{mid}: started; computer players for seats {filled:?}"));
                    self.deliver_multi(mid, out, now);
                    // Their seats are the computer's: the match is off for
                    // them, and its end will not name them.
                    self.free_players(mid, &missing, MatchResult::Cancelled, None, now);
                }
                Err(e) => {
                    self.log(&format!("{mid}: cancelled: {e}"));
                    self.finish_multi(mid, None, now);
                    self.remove_multi(mid);
                }
            }
            return true;
        }
        if let Some((outcome, cause)) = h.game.ended() {
            match h.ended_at {
                None => {
                    self.finish_multi(mid, Some((outcome, cause)), now);
                    busy = true;
                }
                Some(at) => {
                    let gone = h.seats.iter().all(Option::is_none);
                    let since = now.saturating_sub(at);
                    if (gone && since >= crate::serve::LINGER.as_millis() as u64) || since >= ENDED_KEEP_MS {
                        self.remove_multi(mid);
                    }
                }
            }
        }
        busy
    }

    /// Give players back to the lobby while their match goes on.
    fn free_players(&mut self, mid: &str, names: &[String], result: MatchResult, cause: Option<roses_net::Cause>, now: u64) {
        for name in names {
            if let Some(cid) = self.lobby.conn_of(name) {
                self.lobby_send(cid, LobbyEvent::MatchOver { match_id: mid.to_string(), result, cause }, now);
            }
        }
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let actions = self.lobby.ended(&names);
        self.act(actions, now);
    }

    /// Players knocked out of a running N-player match who have left: they
    /// are done with it (a 2v2 team-mate may play on; a player who walked
    /// away has lost), so they are not busy and signing in leaves them out.
    fn release_seats(&mut self, mid: &str, now: u64) -> bool {
        let Some(h) = self.multis.get_mut(mid) else { return false };
        let mut gone = Vec::new();
        for s in 0..h.game.seat_count() {
            if !h.released[usize::from(s)] && h.game.left_after_knockout(s) {
                h.released[usize::from(s)] = true;
                gone.push(h.players[usize::from(s)].clone());
            }
        }
        self.free_players(mid, &gone, MatchResult::Lost, Some(roses_net::Cause::Abandoned), now);
        !gone.is_empty()
    }

    /// An N-player match ended (or was cancelled): log it, keep its record,
    /// tell its players (won: their team won), free them in the lobby.
    fn finish_multi(&mut self, mid: &str, end: Option<(roses_core::duel::Outcome, roses_net::Cause)>, now: u64) {
        let Some(h) = self.multis.get_mut(mid) else { return };
        h.ended_at = Some(now);
        let players = h.players.clone();
        let released = h.released.clone();
        let infos = h.game.seat_infos();
        if let Some((outcome, cause)) = end {
            let shown: Vec<String> = infos.iter().map(|s| s.name.clone()).collect();
            let half = shown.len() / 2;
            let mode = h.game.settings().mode;
            let row = MatchRow {
                id: mid.to_string(),
                players: [shown[..half].join(", "), shown[half..].join(", ")],
                started: h.created_wall,
                ended: accounts::now_seconds(),
                outcome: match outcome.winner {
                    Some(w) => format!("{} won ({:?}, {mode:?})", shown[usize::from(w)], outcome.reason),
                    None => format!("draw ({:?}, {mode:?})", outcome.reason),
                },
                cause: format!("{cause:?}"),
                revisions: h.game.revision() as i64,
            };
            let replay = h.game.replay().clone();
            let line = format!("{mid}: ended, {} after {} revisions", row.outcome, row.revisions);
            if let Err(e) = self.accounts.record_match(&row) {
                self.log(&format!("{mid}: could not log the match: {e}"));
            }
            if let Some(dir) = self.settings.replay_dir.clone() {
                let path = dir.join(format!("{mid}.json"));
                let written = std::fs::create_dir_all(&dir)
                    .and_then(|_| std::fs::write(&path, serde_json::to_vec(&replay).unwrap_or_default()));
                if let Err(e) = written {
                    self.log(&format!("{mid}: replay not written: {e}"));
                }
            }
            self.log(&line);
        }
        for (seat, name) in players.iter().enumerate() {
            if name == COMPUTER_NAME || released[seat] {
                continue;
            }
            let result = match end {
                None => MatchResult::Cancelled,
                Some((o, _)) => match o.winner {
                    None => MatchResult::Draw,
                    Some(w) if infos[usize::from(w)].team == infos[seat].team => MatchResult::Won,
                    Some(_) => MatchResult::Lost,
                },
            };
            if let Some(cid) = self.lobby.conn_of(name) {
                let event = LobbyEvent::MatchOver { match_id: mid.to_string(), result, cause: end.map(|(_, c)| c) };
                self.lobby_send(cid, event, now);
            }
        }
        let names: Vec<&str> =
            players.iter().zip(&released).filter(|(n, r)| *n != COMPUTER_NAME && !**r).map(|(n, _)| n.as_str()).collect();
        let actions = self.lobby.ended(&names);
        self.act(actions, now);
    }

    fn remove_multi(&mut self, mid: &str) {
        let Some(h) = self.multis.remove(mid) else { return };
        for t in &h.tickets {
            self.multi_tickets.remove(t);
        }
        self.watch_tickets.retain(|_, m| m != mid);
        for cid in h.seats.into_iter().flatten().chain(h.spectators) {
            if let Some(mut c) = self.conns.remove(&cid) {
                c.conn.close();
            }
        }
    }

    /// N-player matches being hosted (tests).
    pub fn multi_count(&self) -> usize {
        self.multis.len()
    }
}
