//! One authoritative online match as a pure state machine: it owns the
//! `roses_core` duel, decides every intention, numbers accepted changes,
//! filters what each seat receives and applies the online policy. Time is
//! an explicit tick argument (the TCP loop passes milliseconds; tests pass
//! whatever they like), so nothing depends on a frame rate or wall clock.
//!
//! Spectators (a port addition) get the view of nobody: every accepted
//! revision is filtered once for them (`roses_net::spectator_view`,
//! `spectator_events`) into the match's spectator log, which releases the
//! frames as `Update`s (after the optional delay of whole turns) for the
//! hub to fan out to every spectator ([`OnlineMatch::take_spectator_messages`]).
//! The log keeps only frames not yet released and the last released one
//! (a late joiner's snapshot). The server-only replay is never part of it.

use crate::replay::{state_hash, MatchSetup, Replay, ReplayAction, ReplayEntry, REPLAY_SCHEMA};
use roses_core::duel::{Catalogue, Duel, DuelCommand, DuelError, DuelEvent, Outcome, Pick};
use roses_net::{
    check_deck, data_fingerprint, player_view_with, seen_events_with, spectator_events, spectator_view, Actor, Cause,
    ClientMessage, Compat, Decision, DeckSubmission, Join, MatchOptions, MatchStatus, OnlinePolicy, PlayerView, Press,
    PressResponse, Refusal, Rejection, RulesTables, SeenEvent, ServerMessage, Snapshot,
};
use std::collections::VecDeque;

/// Decisions remembered per seat for answering retransmissions.
pub const REMEMBERED_DECISIONS: usize = 256;
/// Most revisions one streamed press may claim (one press sends at most a
/// few intentions).
pub const PRESS_REVISIONS: u64 = 16;

/// Messages to deliver, in order, by seat.
pub type Outbox = Vec<(u8, ServerMessage)>;

/// One accepted revision as spectators see it (the spectator log): filtered
/// once per match, whatever the number of spectators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpectatorFrame {
    pub revision: u64,
    pub actor: Actor,
    pub cause: Cause,
    pub events: Vec<SeenEvent>,
    pub view: PlayerView,
    /// Turns begun before this revision's state (0 = the first turn).
    pub turn: u32,
}

impl SpectatorFrame {
    fn update(&self) -> ServerMessage {
        ServerMessage::Update {
            revision: self.revision,
            actor: self.actor,
            cause: self.cause,
            events: self.events.clone(),
            view: self.view.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct Seat {
    joined: bool,
    connected: bool,
    token: String,
    last_seq: u64,
    decisions: VecDeque<(u64, Decision)>,
    timeouts: u8,
    absent_since: Option<u64>,
}

/// Where the decks come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeckRule {
    /// The setup's own decks (tests, the import's first duel).
    Fixed,
    /// Each seat submits its deck in its `Hello`; the duel is built when
    /// both seats have joined. Until then no state is sent to anyone.
    Submitted,
}

#[derive(Clone, Debug)]
pub struct OnlineMatch {
    id: String,
    deck_rule: DeckRule,
    /// The seats' checked decks (server-only; never sent to anyone).
    submitted: [Option<DeckSubmission>; 2],
    compat: Compat,
    policy: OnlinePolicy,
    cards: Catalogue,
    /// The executable's rules tables (leader abilities, Destiny Draw rows).
    tables: RulesTables,
    /// The Custom Duel switches this server plays.
    options: MatchOptions,
    /// Seats whose client streams its presses.
    presence: [bool; 2],
    duel: Duel,
    revision: u64,
    replay: Replay,
    seats: [Seat; 2],
    started: bool,
    ended: Option<(Outcome, Cause)>,
    now: u64,
    turn_clock: u64,
    turn_key: (u8, u8),
    /// Spectating: frames not yet released (the delay), the last released
    /// one (a late spectator's snapshot), what the hub sends every
    /// spectator, the delay in turns, turns begun so far, spectators now and
    /// the seats whose client reads the "N watching" count.
    spectator_log: VecDeque<SpectatorFrame>,
    spectator_shown: Option<SpectatorFrame>,
    spectator_out: Vec<ServerMessage>,
    spectator_delay: u8,
    turns: u32,
    watchers: u16,
    reads_watchers: [bool; 2],
}

fn rejection(e: DuelError) -> Rejection {
    match e {
        DuelError::Finished => Rejection::Finished,
        DuelError::NotYourTurn => Rejection::NotYourTurn,
        DuelError::InvalidCommand(m) => Rejection::InvalidCommand { message: m.into() },
        DuelError::Illegal(m) => Rejection::Illegal { message: m.into() },
        // The detail may name a hidden card; the server keeps it.
        DuelError::Unsupported(_) => Rejection::Unsupported,
    }
}

/// Constant-time-ish token comparison.
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl OnlineMatch {
    /// A match waiting for two seats. `tokens` are the seats' secret
    /// resume tokens (generate them with [`crate::secret_token`]).
    pub fn new(
        id: impl Into<String>,
        setup: MatchSetup,
        cards: Catalogue,
        policy: OnlinePolicy,
        tokens: [String; 2],
    ) -> Result<Self, String> {
        let tables = RulesTables::empty();
        let duel = setup.build(&cards).map_err(|e| e.to_string())?;
        let compat = Compat::current(data_fingerprint(&cards, &tables));
        let id = id.into();
        let seat = |token: String| Seat {
            joined: false,
            connected: false,
            token,
            last_seq: 0,
            decisions: VecDeque::new(),
            timeouts: 0,
            absent_since: None,
        };
        let [t0, t1] = tokens;
        Ok(Self {
            deck_rule: DeckRule::Fixed,
            submitted: [None, None],
            replay: Replay {
                schema: REPLAY_SCHEMA.into(),
                match_id: id.clone(),
                compat: compat.clone(),
                setup,
                entries: Vec::new(),
                final_state: state_hash(&duel),
            },
            turn_key: (duel.round, duel.active),
            id,
            compat,
            policy,
            cards,
            tables,
            options: MatchOptions::default(),
            presence: [false; 2],
            duel,
            revision: 0,
            seats: [seat(t0), seat(t1)],
            started: false,
            ended: None,
            now: 0,
            turn_clock: 0,
            spectator_log: VecDeque::new(),
            spectator_shown: None,
            spectator_out: Vec::new(),
            spectator_delay: 0,
            turns: 0,
            watchers: 0,
            reads_watchers: [false; 2],
        })
    }

    /// Spectators receive a turn's revisions only once `turns` later turns
    /// have begun (0, the default: at once, with the seats' presses).
    pub fn with_spectator_delay(mut self, turns: u8) -> Self {
        self.spectator_delay = turns;
        self
    }

    pub fn spectator_delay(&self) -> u8 {
        self.spectator_delay
    }

    /// The spectators' snapshot (the last released frame, the view of
    /// nobody); none before the match has started.
    pub fn spectator_snapshot(&self) -> Option<Snapshot> {
        let shown = self.spectator_shown.as_ref()?;
        let status = match self.status() {
            // An ended match is shown ended only once everything is out.
            MatchStatus::Ended { .. } if !self.spectator_log.is_empty() => MatchStatus::Running,
            s => s,
        };
        Some(Snapshot {
            revision: shown.revision,
            last_seq: 0,
            status,
            view: shown.view.clone(),
            // Presses are relayed to spectators only without a delay.
            presence: if self.spectator_delay == 0 { self.presence.to_vec() } else { vec![false; 2] },
        })
    }

    /// The round spectators see now (1-based); none before the start.
    pub fn spectator_round(&self) -> Option<u8> {
        self.spectator_shown.as_ref().map(|f| f.view.board.round)
    }

    /// The spectator log's frames not yet released (tests, load checks).
    pub fn spectator_backlog(&self) -> usize {
        self.spectator_log.len()
    }

    /// Messages for every spectator of this match, in order (the hub fans
    /// them out); taken once.
    pub fn take_spectator_messages(&mut self) -> Vec<ServerMessage> {
        std::mem::take(&mut self.spectator_out)
    }

    /// How many spectators watch now: the seats whose client reads it and
    /// every spectator are told when it changes.
    pub fn set_watchers(&mut self, count: u16) -> Outbox {
        if count == self.watchers {
            return Vec::new();
        }
        self.watchers = count;
        self.spectator_out.push(ServerMessage::Watchers { count });
        self.connected_seats()
            .filter(|&s| self.reads_watchers[usize::from(s)])
            .map(|s| (s, ServerMessage::Watchers { count }))
            .collect()
    }

    pub fn watchers(&self) -> u16 {
        self.watchers
    }

    /// The match began: its first state is the spectators' first frame.
    fn start_spectator_log(&mut self) {
        let frame = SpectatorFrame {
            revision: self.revision,
            actor: Actor::Server,
            cause: Cause::Command,
            events: Vec::new(),
            view: spectator_view(&self.duel),
            turn: self.turns,
        };
        self.spectator_log.clear();
        self.spectator_shown = Some(frame);
    }

    /// Release the frames whose delay is over (all of them once the match
    /// has ended).
    fn release_spectator_frames(&mut self) {
        let ended = self.ended.is_some();
        while let Some(f) = self.spectator_log.front() {
            if !ended && f.turn + u32::from(self.spectator_delay) > self.turns {
                break;
            }
            let f = self.spectator_log.pop_front().expect("checked");
            self.spectator_out.push(f.update());
            self.spectator_shown = Some(f);
        }
    }

    /// A match whose seats submit their own decks ([`DeckRule::Submitted`]):
    /// `base` gives the board, first side, rules and seed; its decks and
    /// leaders are replaced by the submitted ones at the start.
    pub fn with_submitted_decks(
        id: impl Into<String>,
        base: MatchSetup,
        cards: Catalogue,
        policy: OnlinePolicy,
        tokens: [String; 2],
    ) -> Result<Self, String> {
        let mut m = Self::new(id, base, cards, policy, tokens)?;
        m.deck_rule = DeckRule::Submitted;
        Ok(m)
    }

    /// Play with the executable's rules tables (they join the rules
    /// fingerprint every client must match; the Destiny Draw rows go into
    /// the duel) and the given Custom Duel switches (sent to each seat in
    /// its welcome). Only before the match has started.
    pub fn with_rules(mut self, tables: RulesTables, options: MatchOptions) -> Result<Self, String> {
        if self.started {
            return Err("the match has started".into());
        }
        self.compat = Compat::current(data_fingerprint(&self.cards, &tables));
        self.replay.compat = self.compat.clone();
        self.replay.setup.destiny_rows = tables.destiny;
        self.duel = self.replay.setup.build(&self.cards).map_err(|e| e.to_string())?;
        self.replay.final_state = state_hash(&self.duel);
        self.tables = tables;
        self.options = options;
        Ok(self)
    }

    pub fn options(&self) -> MatchOptions {
        self.options
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn deck_rule(&self) -> &DeckRule {
        &self.deck_rule
    }
    pub fn compat(&self) -> &Compat {
        &self.compat
    }
    pub fn duel(&self) -> &Duel {
        &self.duel
    }
    pub fn cards(&self) -> &Catalogue {
        &self.cards
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn ended(&self) -> Option<(Outcome, Cause)> {
        self.ended
    }
    pub fn started(&self) -> bool {
        self.started
    }
    /// Whether `seat` has joined (it can only come back with a resume).
    pub fn joined(&self, seat: u8) -> bool {
        self.seats[usize::from(seat)].joined
    }
    pub fn connected(&self, seat: u8) -> bool {
        self.seats[usize::from(seat)].connected
    }
    /// The server-only match record.
    pub fn replay(&self) -> &Replay {
        &self.replay
    }

    pub fn status(&self) -> MatchStatus {
        if let Some((outcome, cause)) = self.ended {
            return MatchStatus::Ended { outcome, cause };
        }
        if !self.started {
            return MatchStatus::Waiting;
        }
        let absent = [!self.seats[0].connected, !self.seats[1].connected];
        if absent.contains(&true) {
            MatchStatus::Paused { absent }
        } else {
            MatchStatus::Running
        }
    }

    pub fn snapshot(&self, seat: u8) -> Snapshot {
        Snapshot {
            revision: self.revision,
            last_seq: self.seats[usize::from(seat)].last_seq,
            status: self.status(),
            view: self.view(seat),
            presence: self.presence.to_vec(),
        }
    }

    /// What `seat` may see under this match's Card Display switches.
    fn view(&self, seat: u8) -> roses_net::PlayerView {
        player_view_with(&self.duel, seat, self.options.card_display_open)
    }

    fn connected_seats(&self) -> impl Iterator<Item = u8> + '_ {
        (0u8..2).filter(|&s| self.seats[usize::from(s)].connected)
    }

    /// A connection's handshake without a deck (fixed-deck matches and
    /// resumes).
    pub fn hello(&mut self, compat: &Compat, join: &Join, now: u64) -> Result<(u8, Outbox), Refusal> {
        self.hello_with_deck(compat, join, None, now)
    }

    /// A connection's handshake. On success the seat is (re)attached and
    /// the returned messages start with its `Welcome`. With submitted decks
    /// a seat's join must carry a legal deck (checked before the seat is
    /// taken); a resume ignores `deck`.
    pub fn hello_with_deck(
        &mut self,
        compat: &Compat,
        join: &Join,
        deck: Option<&DeckSubmission>,
        now: u64,
    ) -> Result<(u8, Outbox), Refusal> {
        self.hello_streaming(compat, join, deck, false, now)
    }

    /// [`Self::hello_with_deck`] for a client that says whether it streams
    /// its presses (the other seat is told in its snapshots).
    pub fn hello_streaming(
        &mut self,
        compat: &Compat,
        join: &Join,
        deck: Option<&DeckSubmission>,
        presence: bool,
        now: u64,
    ) -> Result<(u8, Outbox), Refusal> {
        self.hello_spectating(compat, join, deck, presence, false, now)
    }

    /// [`Self::hello_streaming`] for a client that says whether it reads the
    /// spectating messages (`spectate`: it is sent the "N watching" count).
    pub fn hello_spectating(
        &mut self,
        compat: &Compat,
        join: &Join,
        deck: Option<&DeckSubmission>,
        presence: bool,
        spectate: bool,
        now: u64,
    ) -> Result<(u8, Outbox), Refusal> {
        compat
            .check(&self.compat)
            .map_err(|detail| Refusal::Incompatible { detail })?;
        if matches!(join, Join::Seat { .. }) && self.deck_rule == DeckRule::Submitted && !self.started {
            let deck = deck.ok_or(Refusal::DeckRequired)?;
            check_deck(deck, &self.cards).map_err(|problem| Refusal::IllegalDeck { problem })?;
        }
        let mut out = self.advance(now);
        let seat = match join {
            Join::Seat { seat } => {
                if self.ended.is_some() {
                    return Err(Refusal::MatchOver);
                }
                let seat = match seat {
                    Some(s) if *s > 1 => return Err(Refusal::SeatTaken),
                    Some(s) if self.seats[usize::from(*s)].joined => return Err(Refusal::SeatTaken),
                    Some(s) => *s,
                    None => (0u8..2)
                        .find(|&s| !self.seats[usize::from(s)].joined)
                        .ok_or(Refusal::MatchFull)?,
                };
                self.seats[usize::from(seat)].joined = true;
                if self.deck_rule == DeckRule::Submitted {
                    self.submitted[usize::from(seat)] = deck.cloned();
                }
                seat
            }
            Join::Resume { token } => (0u8..2)
                .find(|&s| {
                    let st = &self.seats[usize::from(s)];
                    st.joined && same_token(&st.token, token)
                })
                .ok_or(Refusal::BadToken)?,
            // Tickets are the lobby hub's; it turns them into a seat join
            // or a resume (or a spectator) before the match sees them.
            Join::Ticket { .. } | Join::Watch { .. } => return Err(Refusal::BadToken),
        };
        let was_paused = matches!(self.status(), MatchStatus::Paused { .. });
        let presence_changed = self.presence[usize::from(seat)] != presence;
        self.presence[usize::from(seat)] = presence;
        self.reads_watchers[usize::from(seat)] = spectate;
        let st = &mut self.seats[usize::from(seat)];
        st.connected = true;
        st.absent_since = None;
        out.push((
            seat,
            ServerMessage::Welcome {
                match_id: self.id.clone(),
                seat,
                resume_token: st.token.clone(),
                compat: self.compat.clone(),
                policy: self.policy.clone(),
                options: self.options,
                multi: None,
            },
        ));
        if !self.started && self.seats.iter().all(|s| s.joined && s.connected) {
            if self.deck_rule == DeckRule::Submitted {
                if let Err(refusal) = self.build_from_submissions() {
                    let st = &mut self.seats[usize::from(seat)];
                    st.joined = false;
                    st.connected = false;
                    self.submitted[usize::from(seat)] = None;
                    return Err(refusal);
                }
            }
            self.started = true;
            self.turn_clock = 0;
            self.start_spectator_log();
            for s in 0..2 {
                out.push((s, ServerMessage::Snapshot { snapshot: self.snapshot(s) }));
            }
        } else if self.started || self.deck_rule == DeckRule::Fixed {
            out.push((seat, ServerMessage::Snapshot { snapshot: self.snapshot(seat) }));
            // The other seat learns when this one's streaming changed (a
            // different client resumed the seat).
            let other = seat ^ 1;
            if presence_changed && self.started && self.seats[usize::from(other)].connected {
                out.push((other, ServerMessage::Snapshot { snapshot: self.snapshot(other) }));
            }
            if was_paused && matches!(self.status(), MatchStatus::Running) {
                for s in 0..2 {
                    out.push((s, ServerMessage::Resumed));
                }
                self.spectator_out.push(ServerMessage::Resumed);
            }
        }
        if spectate && self.watchers > 0 {
            out.push((seat, ServerMessage::Watchers { count: self.watchers }));
        }
        Ok((seat, out))
    }

    /// The duel from the base setup with both submitted decks (the match
    /// record keeps them; no seat ever receives the other's).
    fn build_from_submissions(&mut self) -> Result<(), Refusal> {
        let [Some(a), Some(b)] = &self.submitted else {
            return Err(Refusal::DeckRequired);
        };
        let setup = self.replay.setup.with_decks([a, b], &self.tables, self.options);
        let duel = setup.build(&self.cards).map_err(|e| Refusal::IllegalDeck {
            problem: roses_net::DeckProblem::Unplayable { reason: e.to_string() },
        })?;
        self.turn_key = (duel.round, duel.active);
        self.replay.setup = setup;
        self.replay.final_state = state_hash(&duel);
        self.duel = duel;
        Ok(())
    }

    /// The seat's connection went away.
    pub fn disconnect(&mut self, seat: u8, now: u64) -> Outbox {
        let mut out = self.advance(now);
        let st = &mut self.seats[usize::from(seat)];
        if !st.connected {
            return out;
        }
        st.connected = false;
        if self.started && self.ended.is_none() {
            st.absent_since = Some(self.now);
            let absent = [!self.seats[0].connected, !self.seats[1].connected];
            let deadline = self.now + self.policy.reconnect_grace;
            for s in self.connected_seats().collect::<Vec<_>>() {
                out.push((s, ServerMessage::Paused { absent, deadline }));
            }
            self.spectator_out.push(ServerMessage::Paused { absent, deadline });
        }
        out
    }

    /// Advance the clock to `now` (policy timeouts).
    pub fn tick(&mut self, now: u64) -> Outbox {
        self.advance(now)
    }

    fn advance(&mut self, now: u64) -> Outbox {
        let dt = now.saturating_sub(self.now);
        self.now = self.now.max(now);
        let mut out = Vec::new();
        if !self.started || self.ended.is_some() {
            return out;
        }
        // Absent seats: forfeit once the grace period has passed (the
        // earlier departure first).
        let mut expired: Vec<(u64, u8)> = (0u8..2)
            .filter_map(|s| {
                let since = self.seats[usize::from(s)].absent_since?;
                (self.now.saturating_sub(since) >= self.policy.reconnect_grace).then_some((since, s))
            })
            .collect();
        expired.sort();
        if let Some(&(_, seat)) = expired.first() {
            self.forfeit(seat, Actor::Server, Cause::Abandoned, &mut out);
            return out;
        }
        if !matches!(self.status(), MatchStatus::Running) {
            return out; // paused: the turn clock stops
        }
        self.turn_clock += dt;
        if let Some(limit) = self.policy.turn_limit {
            if self.turn_clock >= limit {
                self.turn_timeout(&mut out);
            }
        }
        out
    }

    fn turn_timeout(&mut self, out: &mut Outbox) {
        let seat = self.duel.active;
        let st = &mut self.seats[usize::from(seat)];
        st.timeouts += 1;
        if st.timeouts >= self.policy.timeouts_before_forfeit {
            self.forfeit(seat, Actor::Server, Cause::RepeatedTimeouts, out);
            return;
        }
        // Close what is open (Circle), then end the turn.
        let mut steps = Vec::new();
        if self.duel.pending.is_some() {
            steps.push(DuelCommand::Pick { pick: Pick::Cancel });
        }
        if self.duel.pending_summon.is_some() {
            steps.push(DuelCommand::CancelSummon);
        }
        steps.push(DuelCommand::EndTurn);
        for command in steps {
            match self.duel.apply(seat, &command, &self.cards) {
                Ok((next, events)) => {
                    self.commit(seat, Actor::Server, Cause::TurnTimeout, ReplayAction::Command { side: seat, command }, next, events, out)
                }
                Err(_) => {
                    self.forfeit(seat, Actor::Server, Cause::TurnTimeout, out);
                    return;
                }
            }
            if self.ended.is_some() {
                return;
            }
        }
        self.turn_clock = 0;
    }

    fn forfeit(&mut self, seat: u8, actor: Actor, cause: Cause, out: &mut Outbox) {
        if let Ok((next, events)) = self.duel.forfeit(seat) {
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
        self.duel = next;
        self.replay.entries.push(ReplayEntry {
            revision: self.revision,
            actor,
            cause,
            action,
        });
        self.replay.final_state = state_hash(&self.duel);
        let key = (self.duel.round, self.duel.active);
        if key != self.turn_key {
            self.turn_key = key;
            self.turn_clock = 0;
            self.turns += 1;
        }
        // The spectators' frame: filtered once, whoever watches.
        self.spectator_log.push_back(SpectatorFrame {
            revision: self.revision,
            actor,
            cause,
            events: spectator_events(&self.duel, Some(side), &events),
            view: spectator_view(&self.duel),
            turn: self.turns,
        });
        for viewer in self.connected_seats().collect::<Vec<_>>() {
            out.push((
                viewer,
                ServerMessage::Update {
                    revision: self.revision,
                    actor,
                    cause,
                    events: seen_events_with(&self.duel, Some(side), viewer, &events, self.options.card_display_open),
                    view: self.view(viewer),
                },
            ));
        }
        if let Some(outcome) = self.duel.outcome {
            self.ended = Some((outcome, cause));
            for viewer in self.connected_seats().collect::<Vec<_>>() {
                out.push((
                    viewer,
                    ServerMessage::Ended {
                        revision: self.revision,
                        outcome,
                        cause,
                    },
                ));
            }
        }
        self.release_spectator_frames();
        if let Some((outcome, cause)) = self.ended {
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

    /// A streamed press of `seat` for the other seat's screen (presentation
    /// only: it changes nothing here). Relayed only when it fits the match:
    /// a press without intentions on the current revision by the acting
    /// seat, or a press whose revisions (`base_revision`+1 ..=
    /// `through_revision`) were all this seat's own; anything else is
    /// dropped. Hidden card ids are removed first.
    fn relay_press(&mut self, seat: u8, mut press: Press) -> Outbox {
        let other = seat ^ 1;
        let spectators = self.watchers > 0 && self.spectator_delay == 0;
        if !self.started || (!self.seats[usize::from(other)].connected && !spectators) {
            return Vec::new();
        }
        let (base, through) = (press.base_revision, press.through_revision);
        let fits = if through > base {
            press.response == PressResponse::Commands
                && through <= self.revision
                && through - base <= PRESS_REVISIONS
                && self.replay.entries[base as usize..through as usize]
                    .iter()
                    .all(|e| e.actor == Actor::Seat { seat })
        } else {
            through == base
                && base == self.revision
                && self.duel.active == seat
                && matches!(self.status(), MatchStatus::Running)
        };
        if !fits {
            return Vec::new();
        }
        if spectators {
            // Against the view of nobody (no hand card's identity, even
            // under OPEN); without a delay the frame shown is the current one.
            if let Some(shown) = &self.spectator_shown {
                let mut p = press.clone();
                roses_net::presence::sanitize(&mut p, &shown.view);
                self.spectator_out.push(ServerMessage::Presence { seat, press: p });
            }
        }
        if !self.seats[usize::from(other)].connected {
            return Vec::new();
        }
        let view = self.view(other);
        roses_net::presence::sanitize(&mut press, &view);
        vec![(other, ServerMessage::Presence { seat, press })]
    }

    /// One message from an attached seat.
    pub fn handle(&mut self, seat: u8, message: ClientMessage, now: u64) -> Outbox {
        let mut out = self.advance(now);
        let (seq, command) = match message {
            ClientMessage::Hello { .. } => {
                out.push((
                    seat,
                    ServerMessage::ProtocolError {
                        message: "already joined".into(),
                    },
                ));
                return out;
            }
            ClientMessage::Sync => {
                out.push((seat, ServerMessage::Snapshot { snapshot: self.snapshot(seat) }));
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
            let decision = st
                .decisions
                .iter()
                .find(|(s, _)| *s == seq)
                .map(|(_, d)| d.clone())
                .unwrap_or(Decision::Forgotten);
            out.push((seat, ServerMessage::Duplicate { seq, decision }));
            return out;
        }
        let refusal = if !self.started {
            Some(Rejection::NotStarted)
        } else if self.ended.is_some() {
            Some(Rejection::Finished)
        } else {
            None
        };
        let result = match (refusal, command) {
            (Some(r), _) => Err(r),
            // A forfeit is accepted while paused and off-turn.
            (None, None) => self.duel.forfeit(seat).map_err(rejection).map(|r| (r, ReplayAction::Forfeit { side: seat }, Cause::Forfeit)),
            (None, Some(_)) if !matches!(self.status(), MatchStatus::Running) => Err(Rejection::Paused),
            (None, Some(_)) if seat != self.duel.active => Err(Rejection::NotYourTurn),
            (None, Some((base, _))) if base != self.revision => Err(Rejection::Stale {
                base_revision: base,
                revision: self.revision,
            }),
            (None, Some((_, command))) => self
                .duel
                .apply(seat, &command, &self.cards)
                .map_err(rejection)
                .map(|r| (r, ReplayAction::Command { side: seat, command }, Cause::Command)),
        };
        match result {
            Err(reason) => {
                self.remember(seat, seq, Decision::Rejected { reason: reason.clone() });
                out.push((
                    seat,
                    ServerMessage::Rejected {
                        seq,
                        revision: self.revision,
                        reason,
                    },
                ));
            }
            Ok(((next, events), action, cause)) => {
                let revision = self.revision + 1;
                self.remember(seat, seq, Decision::Accepted { revision });
                self.seats[usize::from(seat)].timeouts = 0;
                out.push((seat, ServerMessage::Accepted { seq, revision }));
                self.commit(seat, Actor::Seat { seat }, cause, action, next, events, &mut out);
            }
        }
        out
    }
}
