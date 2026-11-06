//! One authoritative N-player match (MODES, a port addition: the
//! 4-player free-for-all and 2v2 of `roses_core::duel::multi`), as a pure
//! state machine like [`crate::online::OnlineMatch`]: it owns the duel,
//! decides every intention, numbers accepted changes and filters what each
//! seat (and each spectator) receives. Differences from a 2-player match
//! (MULTIPLAYER-DESIGN.md §2.8, §3):
//!
//! - four seats, each a player (who joins with a lobby ticket and their
//!   deck) or the server's computer player, which plays in-process with
//!   the ported computer modules on a 2-side projection
//!   (`roses_core::duel::multi::computer`);
//! - the match never pauses: a disconnected player's turns are skipped,
//!   and after the reconnect grace (or three expired turns in a row) the
//!   player is knocked out (`Duel::forfeit_with`), and play goes on; with
//!   nobody left to play (every living seat a player who is away) the turns
//!   wait for a return or the grace instead of racing to the round limit,
//!   and once no player is left at all the computers leave too, so a match
//!   of computers only never runs on with nobody there;
//! - the turn clock is the room's (`RoomSettings::turn_seconds`);
//! - presses are relayed to every other seat;
//! - spectators get every revision at once (no delay).
//!
//! The record ([`MultiReplay`]) stays on the server, like the 2-player one.

use crate::online::{Outbox, PRESS_REVISIONS, REMEMBERED_DECISIONS};
use crate::replay::{state_hash, ReplayAction, ReplayEntry};
use roses_core::duel::ai::weevil;
use roses_core::duel::multi::computer::{CatalogueFusions, MultiComputer};
use roses_core::duel::script::DestinyRows;
use roses_core::duel::{
    build_decks, Catalogue, Duel, DuelCommand, DuelError, DuelEvent, MultiRules, MultiSetup, Outcome, Pick,
    PAD_CONTROLLER,
};
use roses_net::lobby::RoomSettings;
use roses_net::{
    check_deck, data_fingerprint, player_view_with, seen_events_for, spectator_events, spectator_view, Actor, Cause,
    ClientMessage, Compat, Decision, DeckSubmission, Join, MatchOptions, MatchStatus, MultiWelcome, OnlinePolicy,
    PlayerView, Press, PressResponse, Refusal, Rejection, RulesTables, SeatInfo, ServerMessage, Snapshot, Viewer,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

pub const MULTI_REPLAY_SCHEMA: &str = "roses-online-multi-replay-1";

/// Commands a computer seat may have refused in one turn before it leaves.
const COMPUTER_STALLS: u32 = 64;

/// Everything that determines an N-player match's initial duel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiMatchSetup {
    pub seed: u64,
    pub terrain: Vec<u8>,
    pub leaders: Vec<u16>,
    /// Deck lists in stored order (shuffled at match start).
    pub deck_sources: Vec<Vec<u16>>,
    pub first: u8,
    pub leader_abilities: Vec<Vec<u8>>,
    pub leader_ranks: Vec<u8>,
    #[serde(default)]
    pub destiny_rows: Option<DestinyRows>,
    pub rules: MultiRules,
}

impl MultiMatchSetup {
    /// The initial authoritative duel (every side pad-controlled).
    pub fn build(&self, cards: &Catalogue) -> Result<Duel, DuelError> {
        let mut rng = self.seed;
        let sources: Vec<&[u16]> = self.deck_sources.iter().map(Vec::as_slice).collect();
        let decks = build_decks(&mut rng, &sources);
        let n = self.leaders.len();
        let (mut duel, _) = Duel::new_multi(
            MultiSetup {
                rules: self.rules,
                terrain: self.terrain.clone(),
                leaders: self.leaders.clone(),
                decks,
                first: self.first,
                leader_abilities: self.leader_abilities.clone(),
                controllers: vec![PAD_CONTROLLER; n],
            },
            cards,
        )?;
        duel.seed_rng(rng);
        duel.leader_ranks = self.leader_ranks.clone();
        if let Some(rows) = self.destiny_rows {
            duel.set_destiny_rows(rows);
        }
        Ok(duel)
    }
}

/// The server's record of an N-player match (server-only: it holds the
/// seed and every deck).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiReplay {
    pub schema: String,
    pub match_id: String,
    pub compat: Compat,
    /// The seats (names, computer, team) and the room settings.
    pub seats: Vec<SeatInfo>,
    pub settings: RoomSettings,
    /// None until the match started.
    pub setup: Option<MultiMatchSetup>,
    pub entries: Vec<ReplayEntry>,
    pub final_state: String,
}

/// Re-run an N-player record: every entry must apply, in order, and the
/// final state must match the recorded hash.
pub fn replay_multi(record: &MultiReplay, cards: &Catalogue, tables: &RulesTables) -> Result<Duel, String> {
    if record.schema != MULTI_REPLAY_SCHEMA {
        return Err(format!("unsupported replay schema {}", record.schema));
    }
    Compat::current(data_fingerprint(cards, tables))
        .check(&record.compat)
        .map_err(|e| format!("incompatible replay: {e:?}"))?;
    let setup = record.setup.as_ref().ok_or("the match never started")?;
    let mut duel = setup.build(cards).map_err(|e| e.to_string())?;
    for (i, entry) in record.entries.iter().enumerate() {
        if entry.revision != i as u64 + 1 {
            return Err(format!("entry {i} has revision {}", entry.revision));
        }
        let result = match &entry.action {
            ReplayAction::Command { side, command } => duel.apply(*side, command, cards),
            ReplayAction::Forfeit { side } => duel.forfeit_with(*side, cards),
        };
        duel = result.map_err(|e| format!("revision {} does not apply: {e}", entry.revision))?.0;
    }
    if state_hash(&duel) != record.final_state {
        return Err("the final state differs from the record".into());
    }
    Ok(duel)
}

/// One seat of an N-player match as the server arranged it.
#[derive(Clone, Debug)]
pub struct MultiSeatPlan {
    pub info: SeatInfo,
    /// A computer seat's deck (players bring their own when they join).
    pub deck: Option<DeckSubmission>,
    pub token: String,
}

#[derive(Clone, Debug)]
struct Seat {
    info: SeatInfo,
    token: String,
    joined: bool,
    connected: bool,
    deck: Option<DeckSubmission>,
    last_seq: u64,
    decisions: VecDeque<(u64, Decision)>,
    timeouts: u8,
    absent_since: Option<u64>,
    presence: bool,
    reads_watchers: bool,
}

#[derive(Clone)]
pub struct MultiMatch {
    id: String,
    compat: Compat,
    cards: Catalogue,
    tables: RulesTables,
    computer_tables: Option<Arc<weevil::Tables>>,
    settings: RoomSettings,
    policy: OnlinePolicy,
    seed: u64,
    terrain: Vec<u8>,
    seats: Vec<Seat>,
    duel: Option<Duel>,
    revision: u64,
    replay: MultiReplay,
    ended: Option<(Outcome, Cause)>,
    now: u64,
    turn_clock: u64,
    turn_key: (u8, u8),
    computers: Vec<Option<MultiComputer>>,
    /// Milliseconds between two computer commands (the others see them
    /// play), and the time since the last one.
    computer_delay: u64,
    computer_wait: u64,
    computer_stalls: u32,
    spectator_out: Vec<ServerMessage>,
    watchers: u16,
    /// What `advance` produced for a handshake that was then refused: the
    /// next `advance` hands it on, so no seat misses a revision.
    undelivered: Outbox,
}

fn rejection(e: DuelError) -> Rejection {
    match e {
        DuelError::Finished => Rejection::Finished,
        DuelError::NotYourTurn => Rejection::NotYourTurn,
        DuelError::InvalidCommand(m) => Rejection::InvalidCommand { message: m.into() },
        DuelError::Illegal(m) => Rejection::Illegal { message: m.into() },
        DuelError::Unsupported(_) => Rejection::Unsupported,
    }
}

fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl MultiMatch {
    /// A match waiting for its players (computer seats are ready at once).
    /// `policy` gives the reconnect grace; the turn clock is the room's.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        seats: Vec<MultiSeatPlan>,
        settings: RoomSettings,
        terrain: Vec<u8>,
        seed: u64,
        cards: Catalogue,
        tables: RulesTables,
        computer_tables: Option<Arc<weevil::Tables>>,
        policy: OnlinePolicy,
    ) -> Result<Self, String> {
        if !(3..=4).contains(&seats.len()) || terrain.len() != 49 {
            return Err("an N-player match needs 3 or 4 seats and a 7 x 7 board".into());
        }
        if seats.iter().any(|s| s.info.computer && s.deck.is_none()) {
            return Err("a computer seat needs a deck".into());
        }
        let compat = Compat::current(data_fingerprint(&cards, &tables));
        let id = id.into();
        let policy = OnlinePolicy {
            turn_limit: (settings.turn_seconds > 0).then(|| u64::from(settings.turn_seconds) * 1000),
            ..policy
        };
        let infos: Vec<SeatInfo> = seats.iter().map(|s| s.info.clone()).collect();
        let n = seats.len();
        Ok(Self {
            replay: MultiReplay {
                schema: MULTI_REPLAY_SCHEMA.into(),
                match_id: id.clone(),
                compat: compat.clone(),
                seats: infos,
                settings: settings.clone(),
                setup: None,
                entries: Vec::new(),
                final_state: String::new(),
            },
            seats: seats
                .into_iter()
                .map(|p| Seat {
                    joined: p.info.computer,
                    connected: p.info.computer,
                    info: p.info,
                    token: p.token,
                    deck: p.deck,
                    last_seq: 0,
                    decisions: VecDeque::new(),
                    timeouts: 0,
                    absent_since: None,
                    presence: false,
                    reads_watchers: false,
                })
                .collect(),
            id,
            compat,
            cards,
            tables,
            computer_tables,
            settings,
            policy,
            seed,
            terrain,
            duel: None,
            revision: 0,
            ended: None,
            now: 0,
            turn_clock: 0,
            turn_key: (0, 0),
            computers: vec![None; n],
            computer_delay: 350,
            computer_wait: 0,
            computer_stalls: 0,
            spectator_out: Vec::new(),
            watchers: 0,
            undelivered: Vec::new(),
        })
    }

    /// Milliseconds between two computer commands (tests: 0).
    pub fn with_computer_delay(mut self, ms: u64) -> Self {
        self.computer_delay = ms;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn duel(&self) -> Option<&Duel> {
        self.duel.as_ref()
    }
    pub fn started(&self) -> bool {
        self.duel.is_some()
    }
    pub fn ended(&self) -> Option<(Outcome, Cause)> {
        self.ended
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn replay(&self) -> &MultiReplay {
        &self.replay
    }
    pub fn settings(&self) -> &RoomSettings {
        &self.settings
    }
    pub fn seat_infos(&self) -> Vec<SeatInfo> {
        self.seats.iter().map(|s| s.info.clone()).collect()
    }
    pub fn seat_count(&self) -> u8 {
        self.seats.len() as u8
    }
    pub fn joined(&self, seat: u8) -> bool {
        self.seats.get(usize::from(seat)).is_some_and(|s| s.joined)
    }
    pub fn connected(&self, seat: u8) -> bool {
        self.seats.get(usize::from(seat)).is_some_and(|s| s.connected)
    }
    /// A player knocked out of a running match with no connection left: the
    /// match no longer needs them, the lobby may have them back.
    pub fn left_after_knockout(&self, seat: u8) -> bool {
        let Some(st) = self.seats.get(usize::from(seat)) else { return false };
        self.ended.is_none()
            && st.joined
            && !st.connected
            && !st.info.computer
            && self.duel.as_ref().is_some_and(|d| !d.alive(seat))
    }
    pub fn token(&self, seat: u8) -> Option<&str> {
        self.seats.get(usize::from(seat)).map(|s| s.token.as_str())
    }
    pub fn watchers(&self) -> u16 {
        self.watchers
    }
    /// The duel's round (1-based) once started.
    pub fn round(&self) -> Option<u8> {
        self.duel.as_ref().map(|d| d.round)
    }

    pub fn welcome_multi(&self) -> MultiWelcome {
        MultiWelcome { seats: self.seat_infos(), settings: self.settings.clone() }
    }

    pub fn status(&self) -> MatchStatus {
        match (self.ended, &self.duel) {
            (Some((outcome, cause)), _) => MatchStatus::Ended { outcome, cause },
            (None, None) => MatchStatus::Waiting,
            (None, Some(_)) => MatchStatus::Running,
        }
    }

    fn view(&self, seat: u8) -> Option<PlayerView> {
        self.duel.as_ref().map(|d| player_view_with(d, seat, [false; 2]))
    }

    pub fn snapshot(&self, seat: u8) -> Option<Snapshot> {
        Some(Snapshot {
            revision: self.revision,
            last_seq: self.seats[usize::from(seat)].last_seq,
            status: self.status(),
            view: self.view(seat)?,
            presence: self.seats.iter().map(|s| s.presence).collect(),
        })
    }

    /// The spectators' snapshot (the view of nobody); none before the start.
    pub fn spectator_snapshot(&self) -> Option<Snapshot> {
        let duel = self.duel.as_ref()?;
        Some(Snapshot {
            revision: self.revision,
            last_seq: 0,
            status: self.status(),
            view: spectator_view(duel),
            presence: self.seats.iter().map(|s| s.presence).collect(),
        })
    }

    /// Messages for every spectator, in order; taken once.
    pub fn take_spectator_messages(&mut self) -> Vec<ServerMessage> {
        std::mem::take(&mut self.spectator_out)
    }

    /// How many spectators watch now.
    pub fn set_watchers(&mut self, count: u16) -> Outbox {
        if count == self.watchers {
            return Vec::new();
        }
        self.watchers = count;
        self.spectator_out.push(ServerMessage::Watchers { count });
        self.connected_seats()
            .filter(|&s| self.seats[usize::from(s)].reads_watchers)
            .map(|s| (s, ServerMessage::Watchers { count }))
            .collect()
    }

    fn connected_seats(&self) -> impl Iterator<Item = u8> + '_ {
        (0..self.seats.len() as u8).filter(|&s| {
            let st = &self.seats[usize::from(s)];
            st.connected && !st.info.computer
        })
    }

    /// A connection's handshake (the hub turns a ticket into
    /// `Join::Seat { seat: Some(s) }` the first time and a resume after).
    pub fn hello(
        &mut self,
        compat: &Compat,
        join: &Join,
        deck: Option<&DeckSubmission>,
        presence: bool,
        spectate: bool,
        now: u64,
    ) -> Result<(u8, Outbox), Refusal> {
        compat.check(&self.compat).map_err(|detail| Refusal::Incompatible { detail })?;
        let mut out = self.advance(now);
        let seat = match self.take_seat(join, deck) {
            Ok(seat) => seat,
            Err(refusal) => {
                // The clock moved for everyone else: they still get it.
                self.undelivered = out;
                return Err(refusal);
            }
        };
        let st = &mut self.seats[usize::from(seat)];
        st.connected = true;
        st.absent_since = None;
        st.presence = presence;
        st.reads_watchers = spectate;
        out.push((
            seat,
            ServerMessage::Welcome {
                match_id: self.id.clone(),
                seat,
                resume_token: st.token.clone(),
                compat: self.compat.clone(),
                policy: self.policy.clone(),
                options: MatchOptions::default(),
                multi: Some(self.welcome_multi()),
            },
        ));
        if self.duel.is_none() {
            if self.seats.iter().all(|s| s.joined) {
                if let Err(e) = self.start() {
                    let st = &mut self.seats[usize::from(seat)];
                    st.joined = false;
                    st.connected = false;
                    st.deck = None;
                    return Err(Refusal::IllegalDeck { problem: roses_net::DeckProblem::Unplayable { reason: e } });
                }
                for s in self.connected_seats().collect::<Vec<_>>() {
                    if let Some(snapshot) = self.snapshot(s) {
                        out.push((s, ServerMessage::Snapshot { snapshot }));
                    }
                }
            }
        } else if let Some(snapshot) = self.snapshot(seat) {
            out.push((seat, ServerMessage::Snapshot { snapshot }));
        }
        if spectate && self.watchers > 0 {
            out.push((seat, ServerMessage::Watchers { count: self.watchers }));
        }
        Ok((seat, out))
    }

    /// The seat a handshake takes or resumes.
    fn take_seat(&mut self, join: &Join, deck: Option<&DeckSubmission>) -> Result<u8, Refusal> {
        match join {
            Join::Seat { seat: Some(s) } => {
                if self.ended.is_some() {
                    return Err(Refusal::MatchOver);
                }
                let st = self.seats.get(usize::from(*s)).ok_or(Refusal::SeatTaken)?;
                if st.joined || st.info.computer {
                    return Err(Refusal::SeatTaken);
                }
                if self.duel.is_some() {
                    // A seat the server gave to a computer at the start.
                    return Err(Refusal::MatchOver);
                }
                let deck = deck.ok_or(Refusal::DeckRequired)?;
                check_deck(deck, &self.cards).map_err(|problem| Refusal::IllegalDeck { problem })?;
                let st = &mut self.seats[usize::from(*s)];
                st.joined = true;
                st.deck = Some(deck.clone());
                Ok(*s)
            }
            Join::Resume { token } => (0..self.seats.len() as u8)
                .find(|&s| {
                    let st = &self.seats[usize::from(s)];
                    st.joined && !st.info.computer && same_token(&st.token, token)
                })
                .ok_or(Refusal::BadToken),
            Join::Seat { seat: None } | Join::Ticket { .. } | Join::Watch { .. } => Err(Refusal::BadToken),
        }
    }

    /// Seats whose player has not joined get the server's computer (with
    /// these decks and names, in seat order), and the match starts. Used
    /// when the start deadline passes; returns the seats filled.
    pub fn fill_with_computers(&mut self, mut decks: impl FnMut() -> (String, DeckSubmission)) -> Result<(Vec<u8>, Outbox), String> {
        if self.duel.is_some() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut filled = Vec::new();
        for (s, st) in self.seats.iter_mut().enumerate() {
            if !st.joined {
                let (name, deck) = decks();
                st.info.computer = true;
                st.info.name = name;
                st.deck = Some(deck);
                st.joined = true;
                st.connected = true;
                filled.push(s as u8);
            }
        }
        self.replay.seats = self.seat_infos();
        self.start()?;
        let mut out = Vec::new();
        for s in self.connected_seats().collect::<Vec<_>>() {
            if let Some(snapshot) = self.snapshot(s) {
                out.push((s, ServerMessage::Snapshot { snapshot }));
            }
        }
        Ok((filled, out))
    }

    /// Build the duel from every seat's deck.
    fn start(&mut self) -> Result<(), String> {
        let decks: Vec<DeckSubmission> = self
            .seats
            .iter()
            .map(|s| s.deck.clone().ok_or_else(|| "a seat has no deck".to_string()))
            .collect::<Result<_, _>>()?;
        let n = decks.len();
        let setup = MultiMatchSetup {
            seed: self.seed,
            terrain: self.terrain.clone(),
            leaders: decks.iter().map(|d| d.leader).collect(),
            deck_sources: decks.iter().map(|d| d.cards.clone()).collect(),
            first: (self.seed % n as u64) as u8,
            leader_abilities: decks.iter().map(|d| self.tables.leader_abilities(d.leader, d.leader_rank)).collect(),
            leader_ranks: decks.iter().map(|d| d.leader_rank).collect(),
            destiny_rows: self.tables.destiny,
            rules: self.settings.rules(),
        };
        let duel = setup.build(&self.cards).map_err(|e| e.to_string())?;
        self.computers = (0..n as u8)
            .map(|s| self.seats[usize::from(s)].info.computer.then(|| MultiComputer::new(&duel, s)))
            .collect();
        self.turn_key = (duel.round, duel.active);
        self.turn_clock = 0;
        self.replay.final_state = state_hash(&duel);
        self.replay.setup = Some(setup);
        self.replay.seats = self.seat_infos();
        self.duel = Some(duel);
        Ok(())
    }

    /// A seat's connection went away: its turns are skipped from now on
    /// and it is knocked out after the reconnect grace.
    pub fn disconnect(&mut self, seat: u8, now: u64) -> Outbox {
        let mut out = self.advance(now);
        let Some(st) = self.seats.get_mut(usize::from(seat)) else { return out };
        if !st.connected || st.info.computer {
            return out;
        }
        st.connected = false;
        st.absent_since = Some(self.now);
        if self.duel.is_some() && self.ended.is_none() {
            out.extend(self.advance(now));
        }
        out
    }

    /// Advance the clock (grace, turn clock, computer players).
    pub fn tick(&mut self, now: u64) -> Outbox {
        self.advance(now)
    }

    fn advance(&mut self, now: u64) -> Outbox {
        let dt = now.saturating_sub(self.now);
        self.now = self.now.max(now);
        let mut out = std::mem::take(&mut self.undelivered);
        if self.duel.is_none() || self.ended.is_some() {
            return out;
        }
        // Absent players past the grace period are knocked out (the earlier
        // departure first, as in a 2-player match).
        let grace = self.policy.reconnect_grace;
        let mut gone: Vec<(u64, u8)> = (0..self.seats.len() as u8)
            .filter_map(|s| {
                let since = self.seats[usize::from(s)].absent_since?;
                (self.now.saturating_sub(since) >= grace && self.duel.as_ref().is_some_and(|d| d.alive(s))).then_some((since, s))
            })
            .collect();
        gone.sort();
        for (_, s) in gone {
            self.forfeit(s, Actor::Server, Cause::Abandoned, &mut out);
            if self.ended.is_some() {
                return out;
            }
        }
        if self.only_computers_remain() {
            self.computers_leave(&mut out);
            return out;
        }
        self.turn_clock += dt;
        self.computer_wait += dt;
        // A few steps per tick at most (computers with no delay in tests).
        for _ in 0..64 {
            if self.ended.is_some() {
                break;
            }
            let active = self.duel.as_ref().expect("started").active;
            let st = &self.seats[usize::from(active)];
            if st.info.computer {
                if self.computer_wait < self.computer_delay {
                    break;
                }
                self.computer_wait = 0;
                self.computer_step(active, &mut out);
                if self.computer_delay > 0 {
                    break;
                }
                continue;
            }
            if !st.connected {
                if !self.someone_can_play() {
                    // Everyone still in the match is away (a network cut
                    // drops them together): the turns wait for a return or
                    // the grace, and whoever returns gets a full clock.
                    self.turn_clock = 0;
                    break;
                }
                // A disconnected player's turn is skipped at once.
                self.end_turn_for(active, Cause::TurnTimeout, &mut out);
                continue;
            }
            if let Some(limit) = self.policy.turn_limit {
                if self.turn_clock >= limit {
                    self.turn_timeout(active, &mut out);
                    continue;
                }
            }
            break;
        }
        out
    }

    /// A seat still in the match that is a computer or a connected player.
    fn someone_can_play(&self) -> bool {
        self.duel.as_ref().is_some_and(|d| {
            self.seats.iter().enumerate().any(|(s, st)| d.alive(s as u8) && (st.info.computer || st.connected))
        })
    }

    /// The match had players and none is left: every one is knocked out and
    /// gone from the screen, so only computers would play on (for up to the
    /// round limit, with those players kept busy in the lobby).
    fn only_computers_remain(&self) -> bool {
        let Some(duel) = self.duel.as_ref() else { return false };
        let mut humans = self.seats.iter().enumerate().filter(|(_, st)| !st.info.computer).peekable();
        humans.peek().is_some() && humans.all(|(s, st)| !st.connected && !duel.alive(s as u8))
    }

    /// End a match of computers only: they leave one by one until one side
    /// is left, those on a team with a player who left first (a player who
    /// walked away does not win because of who stayed).
    fn computers_leave(&mut self, out: &mut Outbox) {
        let teams_with_players: Vec<u8> = self.seats.iter().filter(|s| !s.info.computer).map(|s| s.info.team).collect();
        let mut order: Vec<(bool, u8)> =
            (0..self.seats.len() as u8).map(|s| (!teams_with_players.contains(&self.seats[usize::from(s)].info.team), s)).collect();
        order.sort();
        for (_, s) in order {
            if self.ended.is_some() {
                return;
            }
            if self.duel.as_ref().is_some_and(|d| d.alive(s)) {
                self.forfeit(s, Actor::Server, Cause::Abandoned, out);
            }
        }
    }

    fn computer_step(&mut self, side: u8, out: &mut Outbox) {
        let Some(duel) = self.duel.as_ref() else { return };
        let Some(computer) = self.computers[usize::from(side)].as_mut() else { return };
        let command = match &self.computer_tables {
            Some(t) => computer.decide(&duel, &self.cards, t.as_ref()),
            None => computer.decide(&duel, &self.cards, &CatalogueFusions(&self.cards)),
        };
        let Some(command) = command else { return };
        match duel.apply(side, &command, &self.cards) {
            Ok((next, events)) => {
                self.computer_stalls = 0;
                let action = ReplayAction::Command { side, command };
                self.commit(side, Actor::Seat { seat: side }, Cause::Command, action, next, events, out);
            }
            Err(_) => {
                if let Some(c) = self.computers[usize::from(side)].as_mut() {
                    c.refused(&command);
                }
                self.computer_stalls += 1;
                if self.computer_stalls > COMPUTER_STALLS {
                    // Nothing it tries is accepted (an interaction the port
                    // refuses): the computer leaves the match.
                    self.computer_stalls = 0;
                    self.forfeit(side, Actor::Server, Cause::Forfeit, out);
                }
            }
        }
    }

    /// The server ends `seat`'s turn (closing what is open first); a turn
    /// that cannot end knocks the seat out.
    fn end_turn_for(&mut self, seat: u8, cause: Cause, out: &mut Outbox) {
        let duel = self.duel.as_ref().expect("started");
        let mut steps = Vec::new();
        if duel.pending.is_some() {
            steps.push(DuelCommand::Pick { pick: Pick::Cancel });
        }
        if duel.pending_summon.is_some() {
            steps.push(DuelCommand::CancelSummon);
        }
        steps.push(DuelCommand::EndTurn);
        for command in steps {
            let duel = self.duel.as_ref().expect("started");
            match duel.apply(seat, &command, &self.cards) {
                Ok((next, events)) => {
                    let action = ReplayAction::Command { side: seat, command };
                    self.commit(seat, Actor::Server, cause, action, next, events, out);
                }
                Err(_) => {
                    self.forfeit(seat, Actor::Server, cause, out);
                    return;
                }
            }
            if self.ended.is_some() || self.duel.as_ref().is_some_and(|d| d.active != seat) {
                return;
            }
        }
    }

    fn turn_timeout(&mut self, seat: u8, out: &mut Outbox) {
        let st = &mut self.seats[usize::from(seat)];
        st.timeouts += 1;
        if st.timeouts >= self.policy.timeouts_before_forfeit {
            self.forfeit(seat, Actor::Server, Cause::RepeatedTimeouts, out);
            return;
        }
        self.end_turn_for(seat, Cause::TurnTimeout, out);
        self.turn_clock = 0;
    }

    fn forfeit(&mut self, seat: u8, actor: Actor, cause: Cause, out: &mut Outbox) {
        let Some(duel) = self.duel.as_ref() else { return };
        if let Ok((next, events)) = duel.forfeit_with(seat, &self.cards) {
            self.commit(seat, actor, cause, ReplayAction::Forfeit { side: seat }, next, events, out);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn commit(
        &mut self,
        side: u8,
        actor: Actor,
        cause: Cause,
        action: ReplayAction,
        next: Duel,
        events: Vec<DuelEvent>,
        out: &mut Outbox,
    ) {
        self.revision += 1;
        self.replay.entries.push(ReplayEntry { revision: self.revision, actor, cause, action });
        self.replay.final_state = state_hash(&next);
        let key = (next.round, next.active);
        if key != self.turn_key {
            self.turn_key = key;
            self.turn_clock = 0;
            self.computer_wait = 0;
        }
        // Nobody watching: a later spectator starts from a fresh snapshot.
        if self.watchers > 0 {
            self.spectator_out.push(ServerMessage::Update {
                revision: self.revision,
                actor,
                cause,
                events: spectator_events(&next, Some(side), &events),
                view: spectator_view(&next),
            });
        }
        for viewer in self.connected_seats().collect::<Vec<_>>() {
            out.push((
                viewer,
                ServerMessage::Update {
                    revision: self.revision,
                    actor,
                    cause,
                    events: seen_events_for(&next, Some(side), Viewer::Seat(viewer), &events, [false; 2]),
                    view: player_view_with(&next, viewer, [false; 2]),
                },
            ));
        }
        let outcome = next.outcome;
        self.duel = Some(next);
        if let Some(outcome) = outcome {
            self.ended = Some((outcome, cause));
            for viewer in self.connected_seats().collect::<Vec<_>>() {
                out.push((viewer, ServerMessage::Ended { revision: self.revision, outcome, cause }));
            }
            self.spectator_out.push(ServerMessage::Ended { revision: self.revision, outcome, cause });
        }
    }

    fn remember(&mut self, seat: u8, seq: u64, decision: Decision) {
        let st = &mut self.seats[usize::from(seat)];
        st.last_seq = seq;
        st.decisions.push_back((seq, decision));
        while st.decisions.len() > REMEMBERED_DECISIONS {
            st.decisions.pop_front();
        }
    }

    /// The acting seat's press, for every other seat's screen (and the
    /// spectators'), with what each may not see removed.
    fn relay_press(&mut self, seat: u8, press: Press) -> Outbox {
        let Some(duel) = self.duel.as_ref() else { return Vec::new() };
        let (base, through) = (press.base_revision, press.through_revision);
        let fits = if through > base {
            press.response == PressResponse::Commands
                && through <= self.revision
                && through - base <= PRESS_REVISIONS
                && self.replay.entries[base as usize..through as usize]
                    .iter()
                    .all(|e| e.actor == Actor::Seat { seat })
        } else {
            through == base && base == self.revision && duel.active == seat && self.ended.is_none()
        };
        if !fits {
            return Vec::new();
        }
        if self.watchers > 0 {
            let mut p = press.clone();
            roses_net::presence::sanitize(&mut p, &spectator_view(duel));
            self.spectator_out.push(ServerMessage::Presence { seat, press: p });
        }
        let mut out = Vec::new();
        for other in self.connected_seats().filter(|&s| s != seat).collect::<Vec<_>>() {
            let mut p = press.clone();
            roses_net::presence::sanitize(&mut p, &player_view_with(duel, other, [false; 2]));
            out.push((other, ServerMessage::Presence { seat, press: p }));
        }
        out
    }

    /// One message from an attached seat.
    pub fn handle(&mut self, seat: u8, message: ClientMessage, now: u64) -> Outbox {
        let mut out = self.advance(now);
        let (seq, command) = match message {
            ClientMessage::Hello { .. } => {
                out.push((seat, ServerMessage::ProtocolError { message: "already joined".into() }));
                return out;
            }
            ClientMessage::Sync => {
                if let Some(snapshot) = self.snapshot(seat) {
                    out.push((seat, ServerMessage::Snapshot { snapshot }));
                }
                return out;
            }
            ClientMessage::Presence { press } => {
                out.extend(self.relay_press(seat, press));
                return out;
            }
            ClientMessage::Command { seq, base_revision, command } => (seq, Some((base_revision, command))),
            ClientMessage::Forfeit { seq } => (seq, None),
        };
        let st = &self.seats[usize::from(seat)];
        if seq <= st.last_seq {
            let decision =
                st.decisions.iter().find(|(s, _)| *s == seq).map(|(_, d)| d.clone()).unwrap_or(Decision::Forgotten);
            out.push((seat, ServerMessage::Duplicate { seq, decision }));
            return out;
        }
        let result = match (&self.duel, self.ended, command) {
            (None, _, _) => Err(Rejection::NotStarted),
            (_, Some(_), _) => Err(Rejection::Finished),
            (Some(duel), None, None) => duel
                .forfeit_with(seat, &self.cards)
                .map_err(rejection)
                .map(|r| (r, ReplayAction::Forfeit { side: seat }, Cause::Forfeit)),
            (Some(duel), None, Some(_)) if seat != duel.active => Err(Rejection::NotYourTurn),
            (Some(_), None, Some((base, _))) if base != self.revision => {
                Err(Rejection::Stale { base_revision: base, revision: self.revision })
            }
            (Some(duel), None, Some((_, command))) => duel
                .apply(seat, &command, &self.cards)
                .map_err(rejection)
                .map(|r| (r, ReplayAction::Command { side: seat, command }, Cause::Command)),
        };
        match result {
            Err(reason) => {
                self.remember(seat, seq, Decision::Rejected { reason: reason.clone() });
                out.push((seat, ServerMessage::Rejected { seq, revision: self.revision, reason }));
            }
            Ok(((next, events), action, cause)) => {
                let revision = self.revision + 1;
                self.remember(seat, seq, Decision::Accepted { revision });
                self.seats[usize::from(seat)].timeouts = 0;
                out.push((seat, ServerMessage::Accepted { seq, revision }));
                self.commit(seat, Actor::Seat { seat }, cause, action, next, events, &mut out);
                out.extend(self.advance(now));
            }
        }
        out
    }
}
