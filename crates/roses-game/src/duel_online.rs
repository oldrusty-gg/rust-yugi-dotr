//! The Bevy client's online duel (ONLINE_PLAN.md, after milestone 1): the
//! local seat's pad goes through the unchanged input layer
//! (`roses_core::duel::ui::DuelUi`) on a client-side copy of the duel
//! rebuilt from the seat's view (`roses_net::mirror`); every intention the
//! input layer produces (`Response::Commands`) is sent to the server instead
//! of being applied, and the presentation (board, units, HUD, walks, battle
//! scenes, activations) runs on the copy as each server revision arrives,
//! with the filtered events turned back into engine events.
//!
//! Bevy-free: [`OnlineLink`] owns the connection and the protocol
//! bookkeeping, `duel_play::DuelPlay` drives it once per 25 Hz update, and
//! [`PadBot`] is a pad-level computer player for headless tests.
//!
//! The other seat's turn looks like the original's 2-player duel, where
//! both players watch one screen: each press of the acting seat is
//! streamed (`roses_net::Press`, relayed by the server without hidden card
//! ids) and replayed here through the same input layer and presentation —
//! its cursor walk, the hand row opening, marks and picks, the summon
//! flight, the move selection, details, graveyard and unit picks. A press
//! that sent intentions arrives after its revisions and is presented with
//! them as the local seat's own press would be. The HUD runs with the
//! original's two-pad context (controllers 100 / 101, the match's Leader
//! Ability / Card Display words, R2 on this seat's pad): with HIDE the
//! other side's hand and face-down cards stay hidden (the copy only holds
//! placeholders for them; a hidden summon flies face down). This seat's own
//! cards are always named, on either side's turn: the original hides them
//! until R2 is held only because its two players share a screen
//! (`DuelPlay::hud_own_side`, a port addition).
//!
//! Spectating (a port addition, `--faithful` hides it with the rest of
//! Online Play): [`OnlineLink::watch`] joins with a watch ticket instead of
//! a seat. The server sends the view of nobody (no hand, no face-down card,
//! even when the match is OPEN), so the copy holds placeholders for both
//! hands and every face-down card; both seats' turns are presented as the
//! other seat's turn is to a player (their streamed presses, the shared
//! screen's camera on the active side), with Card Display treated as HIDE
//! on both sides and no R2. The pad is never read except Start, which
//! leaves ([`STOPPED_WATCHING`]). Players are told how many watch.
//!
//! Remaining differences, explicit:
//! - a client that does not stream its presses (the terminal/bot client)
//!   is presented as before: its changes appear at the cells they touch,
//!   without the hand row or flight;
//! - while the local seat's intentions are with the server the pad is not
//!   read (as in the original's animation states), normally a few ms;
//! - the other seat holding R2 under HIDE shows nothing here (its hidden
//!   cards are never sent).

use roses_core::duel::ui::{DuelUi, Mode, Response};
use roses_core::duel::{Catalogue, Duel, DuelEvent};
use roses_net::client::ClientState;
use roses_net::mirror::{duel_events, hidden_card, mirror_duel, reveal};
use roses_net::transport::{Connection, Received};
use roses_net::{
    data_fingerprint, Actor, Cause, ClientMessage, Compat, Decision, DeckSubmission, Join, MatchOptions, MatchStatus,
    PlayerView, Press, PressResponse, Refusal, Rejection, RulesTables, SeenEvent, ServerMessage, Snapshot,
};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

pub type ClientConn = Box<dyn Connection<ClientMessage, ServerMessage> + Send>;
pub type Connector = Box<dyn FnMut() -> std::io::Result<ClientConn> + Send>;

/// Scene param 1 / opponent byte of an online duel: a Custom Duel's (two
/// players: control 101), whose music is track 40.
pub const ONLINE_OPPONENT: u8 = 101;
/// How long a revision of the other seat waits for its streamed press
/// before it is presented without it (a lost or very late press).
pub const PRESS_WAIT: Duration = Duration::from_millis(1500);
/// `DuelPlay::halted` when a spectator pressed Start to leave.
pub const STOPPED_WATCHING: &str = "you stopped watching";
/// How long a dropped connection is retried before the screen gives up (well
/// past the server's reconnect grace, when the seat has been forfeited).
pub const RECONNECT_GIVE_UP: Duration = Duration::from_secs(180);
/// What the menu says then.
pub const LOST_SERVER: &str = "the connection to the server was lost";

/// One server revision as this seat received it.
#[derive(Clone, Debug)]
pub struct Revision {
    pub revision: u64,
    pub actor: Actor,
    pub cause: Cause,
    pub events: Vec<SeenEvent>,
    pub view: PlayerView,
}

/// A local press whose intentions are with the server.
#[derive(Clone, Debug)]
struct Waiting {
    input_before: DuelUi,
    /// The input state the press left (streamed once decided).
    input_after: DuelUi,
    /// The revision the press was made on.
    base: u64,
    key: u16,
    response: Response,
    seqs: Vec<u64>,
}

/// What arrived from the server, in order.
#[derive(Clone, Debug)]
enum Item {
    Revision(Revision, Instant),
    /// The other seat's streamed press.
    Press(Press),
}

/// The other seat's press for the presentation: the press and whether the
/// copy was at its base revision (its key can then be replayed on it).
#[derive(Clone, Debug)]
pub struct RemotePress {
    pub press: Press,
    pub at_base: bool,
}

/// What the presentation does next.
pub enum Batch {
    /// Nothing to present.
    Idle,
    /// The server has not answered the local press yet.
    Waiting,
    /// Revisions to present together: the local press's (with the input
    /// state from before it, the press and the input layer's response), the
    /// other seat's press's (`remote`), or one change by the other seat or
    /// the server.
    Revisions {
        revisions: Vec<Revision>,
        own: Option<(DuelUi, u16, Response)>,
        rejected: Option<Rejection>,
        remote: Option<RemotePress>,
    },
    /// A press of the other seat that sent no intention (cursor, menus).
    Press(RemotePress),
}

/// A `Send` value made `Sync` by only ever handing it out through `&mut`
/// (the link lives in a Bevy resource).
struct Exclusive<T>(std::sync::Mutex<T>);

impl<T> Exclusive<T> {
    fn new(value: T) -> Self {
        Self(std::sync::Mutex::new(value))
    }
    fn get(&mut self) -> &mut T {
        self.0.get_mut().unwrap_or_else(|e| e.into_inner())
    }
    fn into_inner(self) -> T {
        self.0.into_inner().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct OnlineLink {
    conn: Option<Exclusive<ClientConn>>,
    connector: Option<Exclusive<Connector>>,
    pub state: ClientState,
    pub seat: u8,
    compat: Compat,
    queue: VecDeque<Item>,
    waiting: Option<Waiting>,
    decisions: BTreeMap<u64, Decision>,
    /// A snapshot that replaces the copy (start, resume, resync).
    resync: Option<Snapshot>,
    /// The revision the copy shows.
    pub shown_revision: u64,
    pub hidden: u16,
    /// The last notable thing (title line).
    pub note: Option<String>,
    retry_at: Option<Instant>,
    /// The other seat's presses presented (with or without revisions) and
    /// its revisions presented without a press.
    pub remote_presses: u64,
    pub remote_unpressed: u64,
    /// 25 Hz updates presented (`tick`), the update of this seat's last
    /// streamed press and of the other seat's last presented press.
    updates: u64,
    last_sent: u64,
    last_presented: u64,
    /// A spectator's connection (no seat; `seat` is 0, the side the
    /// presentation is drawn from) and its watch ticket (to reconnect).
    pub spectator: bool,
    watch_ticket: Option<String>,
    /// The server will not have this connection back (a spectator: the
    /// match may no longer be watched; a seat: the match is gone), or it
    /// could not be reached for [`RECONNECT_GIVE_UP`]: why.
    pub sent_away: Option<String>,
    /// When the connection dropped (none while connected) and how long it
    /// is retried.
    lost_at: Option<Instant>,
    give_up_after: Duration,
    /// N-player: the seats whose turns the server is skipping, and the
    /// clock their turns are timed on.
    away: roses_net::client::AwaySeats,
    opened: Instant,
}

impl OnlineLink {
    /// Connect, join `join` with `deck` and wait (up to `timeout`) until
    /// the match starts. `connector` is kept for resuming after a dropped
    /// connection.
    pub fn connect(
        mut connector: Connector,
        cards: &Catalogue,
        tables: &RulesTables,
        join: Join,
        deck: Option<DeckSubmission>,
        timeout: Duration,
        mut progress: impl FnMut(&str),
    ) -> Result<Self, String> {
        let compat = Compat::current(data_fingerprint(cards, tables));
        let mut conn = connector().map_err(|e| format!("cannot reach the server: {e}"))?;
        conn.send(&ClientMessage::hello(compat.clone(), join, deck, true))
            .map_err(|e| e.to_string())?;
        let mut link = Self {
            conn: Some(Exclusive::new(conn)),
            connector: Some(Exclusive::new(connector)),
            state: ClientState::new(),
            seat: 0,
            compat,
            queue: VecDeque::new(),
            waiting: None,
            decisions: BTreeMap::new(),
            resync: None,
            shown_revision: 0,
            hidden: hidden_card(cards),
            note: None,
            retry_at: None,
            remote_presses: 0,
            remote_unpressed: 0,
            updates: 0,
            last_sent: 0,
            last_presented: 0,
            spectator: false,
            watch_ticket: None,
            sent_away: None,
            lost_at: None,
            away: Default::default(),
            opened: Instant::now(),
            give_up_after: RECONNECT_GIVE_UP,
        };
        let start = Instant::now();
        let mut said_waiting = false;
        loop {
            link.pump();
            if let Some(reason) = &link.state.refused {
                return Err(reason.plain());
            }
            if link.state.seat.is_some() && !said_waiting {
                said_waiting = true;
                progress(&format!(
                    "joined as seat {}; waiting for the other seat",
                    link.state.seat.unwrap_or(0)
                ));
            }
            if link.state.view.is_some() && link.resync.is_some() {
                link.seat = link.state.seat.ok_or("a snapshot before the welcome")?;
                return Ok(link);
            }
            if link.conn.is_none() {
                return Err("the server closed the connection before the match began".into());
            }
            if start.elapsed() > timeout {
                return Err("the match did not begin in time".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Watch a match with the lobby's watch ticket: wait (up to `timeout`)
    /// for the spectator's welcome and snapshot (the match is under way).
    pub fn watch(
        mut connector: Connector,
        cards: &Catalogue,
        tables: &RulesTables,
        ticket: String,
        timeout: Duration,
    ) -> Result<Self, String> {
        let compat = Compat::current(data_fingerprint(cards, tables));
        let mut conn = connector().map_err(|e| format!("cannot reach the server: {e}"))?;
        conn.send(&ClientMessage::hello(compat.clone(), Join::Watch { ticket: ticket.clone() }, None, false))
            .map_err(|e| e.to_string())?;
        let mut link = Self {
            conn: Some(Exclusive::new(conn)),
            connector: Some(Exclusive::new(connector)),
            state: ClientState::new(),
            seat: 0,
            compat,
            queue: VecDeque::new(),
            waiting: None,
            decisions: BTreeMap::new(),
            resync: None,
            shown_revision: 0,
            hidden: hidden_card(cards),
            note: None,
            retry_at: None,
            remote_presses: 0,
            remote_unpressed: 0,
            updates: 0,
            last_sent: 0,
            last_presented: 0,
            spectator: true,
            watch_ticket: Some(ticket),
            sent_away: None,
            lost_at: None,
            away: Default::default(),
            opened: Instant::now(),
            give_up_after: RECONNECT_GIVE_UP,
        };
        let start = Instant::now();
        loop {
            link.pump();
            if let Some(reason) = &link.state.refused {
                return Err(reason.plain());
            }
            if link.state.spectator && link.state.view.is_some() && link.resync.is_some() {
                return Ok(link);
            }
            if link.conn.is_none() && link.retry_at.is_none() {
                return Err("the server closed the connection".into());
            }
            if start.elapsed() > timeout {
                return Err("the duel could not be watched in time".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The players' names by seat (a spectator's welcome).
    pub fn names(&self) -> Option<[String; 2]> {
        self.state.names.clone()
    }

    /// Spectators watching now (the server's count; 0 from an older server).
    pub fn watchers(&self) -> u16 {
        self.state.watchers
    }

    /// The copy of the duel the match starts with (or the last snapshot).
    pub fn take_snapshot(&mut self, cards: &Catalogue) -> Option<Result<(Duel, u64), String>> {
        let snapshot = self.resync.take()?;
        // Revisions up to the snapshot's are in it (and so are their presses).
        self.queue.retain(|item| match item {
            Item::Revision(r, _) => r.revision > snapshot.revision,
            Item::Press(p) => p.base_revision >= snapshot.revision,
        });
        self.shown_revision = snapshot.revision;
        Some(mirror_duel(&snapshot.view, cards).map(|d| (d, snapshot.revision)))
    }

    fn send(&mut self, message: &ClientMessage) {
        let Some(conn) = self.conn.as_mut() else {
            return;
        };
        if conn.get().send(message).is_err() {
            self.drop_connection("the connection broke while sending");
        }
    }

    fn drop_connection(&mut self, why: &str) {
        if let Some(c) = self.conn.take() {
            c.into_inner().close();
        }
        self.note = Some(format!("{why}; reconnecting"));
        self.retry_at = Some(Instant::now());
        self.lost_at.get_or_insert_with(Instant::now);
    }

    /// Read every waiting message; reconnect (resume) when the connection
    /// dropped, resending undecided intentions.
    pub fn pump(&mut self) {
        if self.conn.is_none() {
            self.reconnect();
        }
        loop {
            let Some(conn) = self.conn.as_mut() else {
                return;
            };
            match conn.get().poll() {
                Received::Message(m) => self.receive(m),
                Received::Empty => return,
                Received::Closed => {
                    if self.state.ended.is_some() {
                        self.conn = None;
                        return;
                    }
                    self.drop_connection("the server connection closed");
                    return;
                }
                Received::Malformed(e) => {
                    self.note = Some(format!("malformed server message: {e}"));
                }
            }
        }
    }

    fn reconnect(&mut self) {
        // A spectator comes back with its watch ticket (a fresh snapshot).
        let join = match (&self.watch_ticket, self.state.token.clone()) {
            (Some(ticket), _) => Join::Watch { ticket: ticket.clone() },
            (None, Some(token)) => Join::Resume { token },
            (None, None) => return,
        };
        let Some(at) = self.retry_at else {
            return;
        };
        if self.state.ended.is_some() || self.sent_away.is_some() || at > Instant::now() {
            return;
        }
        if self.lost_at.is_some_and(|t| t.elapsed() >= self.give_up_after) {
            self.sent_away = Some(LOST_SERVER.into());
            return;
        }
        self.retry_at = Some(Instant::now() + Duration::from_secs(1));
        let Some(connector) = self.connector.as_mut() else {
            return;
        };
        if let Ok(conn) = (connector.get())() {
            self.conn = Some(Exclusive::new(conn));
            let spectator = self.spectator;
            let hello = ClientMessage::hello(self.compat.clone(), join, None, !spectator);
            self.send(&hello);
            if !spectator {
                for m in self.state.resend() {
                    self.send(&m);
                }
            }
            self.note = Some("reconnected".into());
        }
    }

    fn receive(&mut self, m: ServerMessage) {
        let was_active = self.state.view.as_ref().map(|v| v.board.active);
        self.state.apply(&m);
        if matches!(m, ServerMessage::Snapshot { .. }) {
            self.lost_at = None;
        }
        match m {
            ServerMessage::Update { revision, actor, cause, events, view } => {
                let now = self.opened.elapsed().as_millis() as u64;
                self.away.update(self.state.seat, was_active, actor, cause, &view, self.state.turn_limit, now);
                self.queue.push_back(Item::Revision(Revision { revision, actor, cause, events, view }, Instant::now()));
            }
            ServerMessage::Presence { seat, press } => {
                // A spectator presents both seats' presses. The welcome's
                // seat: `self.seat` is not set until `connect` returns, and
                // the first presses can come in with the opening snapshot.
                if Some(seat) != self.state.seat || self.spectator {
                    self.queue.push_back(Item::Press(press));
                }
            }
            ServerMessage::Snapshot { snapshot } => {
                self.away.clear();
                if snapshot.revision >= self.shown_revision || self.resync.is_none() {
                    self.resync = Some(snapshot);
                }
            }
            ServerMessage::Accepted { seq, revision } => {
                self.decisions.insert(seq, Decision::Accepted { revision });
            }
            ServerMessage::Rejected { seq, reason, .. } => {
                self.decisions.insert(seq, Decision::Rejected { reason });
            }
            ServerMessage::Duplicate { seq, decision } => {
                self.decisions.insert(seq, decision);
            }
            ServerMessage::Paused { absent, .. } => {
                let note = format!("paused: seat {} disconnected", if absent[0] { 0 } else { 1 });
                eprintln!("Online: {note}");
                self.note = Some(note);
            }
            ServerMessage::Resumed => {
                eprintln!("Online: resumed");
                self.note = Some("resumed".into());
            }
            ServerMessage::Ended { outcome, cause, .. } => {
                self.note = Some(format!("ended: {outcome:?} ({cause:?})"))
            }
            ServerMessage::Refused { reason } => {
                // Sent away (a player turned spectators off), or a seat's
                // resume refused (the match is gone): no retry. A busy
                // server may have room again.
                let seated = self.state.token.is_some() && !matches!(reason, Refusal::ServerBusy);
                if self.spectator || seated {
                    self.sent_away = Some(reason.plain());
                }
                self.note = Some(format!("refused: {}", reason.plain()))
            }
            // A friends server turned the connection away before reading
            // it (`state.refused` has it as a match refusal).
            ServerMessage::LobbyRefused { .. } => {
                let why = self.state.refused.as_ref().map(|r| r.plain()).unwrap_or_default();
                self.note = Some(why);
            }
            ServerMessage::ProtocolError { message } => self.note = Some(format!("protocol error: {message}")),
            ServerMessage::Welcome { .. } | ServerMessage::Watching { .. } => {}
            ServerMessage::Watchers { count } => {
                eprintln!("Online: {count} watching");
            }
        }
    }

    /// Whether the local seat may press now.
    pub fn may_act(&self) -> bool {
        self.waiting.is_none()
            && self.queue.is_empty()
            && self.resync.is_none()
            && self.conn.is_some()
            && self.state.my_turn()
            && self.state.revision == self.shown_revision
    }

    pub fn paused(&self) -> bool {
        matches!(self.state.status, Some(MatchStatus::Paused { .. }))
    }

    /// Why the duel stands still, for the screen: this game lost the server,
    /// or a seat is gone and the server holds the match for its return.
    pub fn away_note(&self) -> Option<&'static str> {
        if self.lost_at.is_some() {
            Some("RECONNECTING")
        } else if self.paused() {
            Some(if self.spectator { "PLAYER AWAY" } else { "OPPONENT AWAY" })
        } else if self.away.any() {
            // N-player: it may be a team-mate.
            Some("PLAYER AWAY")
        } else {
            None
        }
    }

    /// The match's Custom Duel switches as this screen presents them (the
    /// server's; a spectator knows no hidden card of either side, so both
    /// are presented as HIDE).
    pub fn options(&self) -> MatchOptions {
        let mut o = self.state.options.unwrap_or_default();
        if self.spectator {
            o.card_display_open = [false; 2];
        }
        o
    }

    /// One 25 Hz update of the presentation (the press gaps count these).
    pub fn tick(&mut self) {
        self.updates += 1;
    }

    /// Updates since this seat's previous streamed press (now counted as one).
    fn gap(&mut self) -> u16 {
        let gap = self.updates.saturating_sub(self.last_sent).min(u64::from(u16::MAX)) as u16;
        self.last_sent = self.updates;
        gap
    }

    /// Whether the other seat's press is due: no sooner after its previous
    /// press than it came there (`Press::gap`, capped at 5 s so a stalled
    /// screen catches up).
    fn due(&self, press: &Press) -> bool {
        self.updates >= self.last_presented + u64::from(press.gap.min(250))
    }

    /// Stream a press of this seat that sent no intention (the cursor or a
    /// menu changed) to the other seat's screen.
    pub fn stream_press(&mut self, key: u16, response: &Response, input: &DuelUi) {
        let response = match response {
            Response::Ignored => return,
            Response::Moved => PressResponse::Moved,
            Response::Refused => PressResponse::Refused,
            Response::Commands(_) => PressResponse::Commands,
            Response::Unsupported(_) => PressResponse::Unsupported,
        };
        let base = self.shown_revision;
        let gap = self.gap();
        let press = Press { base_revision: base, through_revision: base, key, response, input: input.clone(), gap };
        self.send(&ClientMessage::Presence { press });
    }

    /// Send the intentions of one press (base revisions: the shown one,
    /// then one more per earlier intention of the same press).
    pub fn submit(&mut self, input_before: DuelUi, input_after: DuelUi, key: u16, response: Response) {
        let Response::Commands(commands) = &response else {
            return;
        };
        let mut seqs = Vec::new();
        for (i, command) in commands.iter().enumerate() {
            let mut m = self.state.command(command.clone());
            if let ClientMessage::Command { base_revision, seq, .. } = &mut m {
                *base_revision = self.shown_revision + i as u64;
                seqs.push(*seq);
                self.state.unacked.insert(*seq, m.clone());
            }
            self.send(&m);
        }
        let base = self.shown_revision;
        self.waiting = Some(Waiting { input_before, input_after, base, key, response, seqs });
    }

    /// Concede (Select's surrender prompt is the in-duel way; this is for
    /// closing the window mid-match).
    pub fn forfeit(&mut self) {
        let m = self.state.forfeit();
        self.send(&m);
    }

    /// Presses whose revisions were already presented without them.
    fn drop_stale_presses(&mut self) {
        while let Some(Item::Press(p)) = self.queue.front() {
            let stale = if p.through_revision > p.base_revision {
                p.through_revision <= self.shown_revision
            } else {
                p.base_revision < self.shown_revision
            };
            if !stale {
                return;
            }
            self.queue.pop_front();
        }
    }

    /// The next thing to present.
    pub fn next_batch(&mut self) -> Batch {
        self.drop_stale_presses();
        if let Some(w) = &self.waiting {
            if w.seqs.iter().any(|s| !self.decisions.contains_key(s)) {
                return Batch::Waiting;
            }
            let mut last = None;
            let mut rejected = None;
            for s in &w.seqs {
                match self.decisions.remove(s) {
                    Some(Decision::Accepted { revision }) => last = Some(revision),
                    Some(Decision::Rejected { reason }) => {
                        rejected.get_or_insert(reason);
                    }
                    _ => {}
                }
            }
            if let Some(r) = last {
                let newest = self.queue.iter().rev().find_map(|item| match item {
                    Item::Revision(q, _) => Some(q.revision),
                    Item::Press(_) => None,
                });
                if newest.is_none_or(|n| n < r) && self.resync.is_none() {
                    // The decision came before its revision: wait for it.
                    for s in &w.seqs {
                        self.decisions.entry(*s).or_insert(Decision::Accepted { revision: r });
                    }
                    return Batch::Waiting;
                }
            }
            let w = self.waiting.take().expect("checked above");
            let mut revisions = Vec::new();
            while let Some(Item::Revision(q, _)) = self.queue.front() {
                if !last.is_some_and(|l| q.revision <= l) {
                    break;
                }
                if let Some(Item::Revision(r, _)) = self.queue.pop_front() {
                    revisions.push(r);
                }
            }
            if let Some(r) = revisions.last() {
                self.shown_revision = r.revision;
            }
            // The other seat sees this press with its revisions (or, all
            // refused, the input state going back).
            let accepted = revisions.last().map(|r| r.revision);
            let gap = self.gap();
            let press = Press {
                base_revision: w.base,
                through_revision: accepted.unwrap_or(w.base),
                key: w.key,
                response: PressResponse::Commands,
                input: if accepted.is_some() { w.input_after.clone() } else { w.input_before.clone() },
                gap,
            };
            self.send(&ClientMessage::Presence { press });
            return Batch::Revisions {
                revisions,
                own: Some((w.input_before, w.key, w.response)),
                rejected,
                remote: None,
            };
        }
        let spectator = self.spectator;
        let other_streams = self.state.presence.get(usize::from((self.seat ^ 1) & 1)).copied().unwrap_or(false);
        match self.queue.front() {
            None => Batch::Idle,
            Some(Item::Press(p)) if p.through_revision == p.base_revision => {
                if !self.due(p) {
                    return Batch::Waiting;
                }
                self.last_presented = self.updates;
                let at_base = p.base_revision == self.shown_revision;
                let Some(Item::Press(press)) = self.queue.pop_front() else { unreachable!() };
                self.remote_presses += 1;
                Batch::Press(RemotePress { press, at_base })
            }
            Some(Item::Press(_)) => {
                // Revisions it claims are not here (a resync): skip it.
                self.queue.pop_front();
                self.next_batch()
            }
            Some(Item::Revision(r, received)) => {
                // A spectator presents every seat's change as a player
                // presents the other seat's.
                let by_other = match r.actor {
                    // Any other seat (N-player: every other seat).
                    Actor::Seat { seat } => (spectator || seat != self.seat) && r.cause == Cause::Command,
                    Actor::Server => false,
                };
                let other_streams = match r.actor {
                    Actor::Seat { seat } if spectator => self.state.presence.get(usize::from(seat)).copied().unwrap_or(false),
                    // N-player (a port addition): the acting seat's own.
                    Actor::Seat { seat } if self.state.presence.len() > 2 => {
                        self.state.presence.get(usize::from(seat)).copied().unwrap_or(false)
                    }
                    _ => other_streams,
                };
                let revision = r.revision;
                let received = *received;
                if by_other && other_streams {
                    // The press that produced it follows its revisions.
                    let found = self.queue.iter().position(|item| {
                        matches!(item, Item::Press(p) if p.base_revision < revision && revision <= p.through_revision)
                    });
                    if let Some(at) = found {
                        if let Some(Item::Press(p)) = self.queue.get(at) {
                            if !self.due(p) {
                                return Batch::Waiting;
                            }
                        }
                        self.last_presented = self.updates;
                        let Some(Item::Press(press)) = self.queue.remove(at) else { unreachable!() };
                        let at_base = press.base_revision == self.shown_revision;
                        let mut revisions = Vec::new();
                        while let Some(Item::Revision(q, _)) = self.queue.front() {
                            if q.revision > press.through_revision {
                                break;
                            }
                            if let Some(Item::Revision(r, _)) = self.queue.pop_front() {
                                revisions.push(r);
                            }
                        }
                        if let Some(r) = revisions.last() {
                            self.shown_revision = r.revision;
                        }
                        self.remote_presses += 1;
                        return Batch::Revisions {
                            revisions,
                            own: None,
                            rejected: None,
                            remote: Some(RemotePress { press, at_base }),
                        };
                    }
                    if received.elapsed() < PRESS_WAIT && self.conn.is_some() {
                        return Batch::Waiting;
                    }
                }
                let Some(Item::Revision(r, _)) = self.queue.pop_front() else { unreachable!() };
                self.shown_revision = r.revision;
                if by_other {
                    self.remote_unpressed += 1;
                }
                Batch::Revisions { revisions: vec![r], own: None, rejected: None, remote: None }
            }
        }
    }

    /// Ask for a fresh snapshot (after a stale or otherwise refused press).
    pub fn sync(&mut self) {
        self.send(&ClientMessage::Sync);
    }

    pub fn ended(&self) -> bool {
        self.state.ended.is_some()
    }
}

/// The events of a batch for the presentation, the cards it revealed put
/// into `before` (the copy from before the batch).
pub fn batch_events(before: &mut Duel, revisions: &[Revision], hidden: u16) -> Vec<DuelEvent> {
    let mut events = Vec::new();
    for r in revisions {
        reveal(before, &r.events);
        events.extend(duel_events(&r.events, hidden));
    }
    events
}

/// The input layer's state made consistent with a new copy: a mode that no
/// longer matches the duel (a new turn, a summon or selection the server
/// closed) returns to the field state; cursors are kept. The input state
/// is the acting side's: the local seat's on its turn, the other seat's
/// (from its streamed presses) on the other turn.
pub fn settle_input(input: &mut DuelUi, duel: &Duel, turn_changed: bool) {
    let summon_open = matches!(input.mode, Mode::Hand(_) | Mode::Confirm(_));
    let selection_open = matches!(input.mode, Mode::SelectUnit(_) | Mode::Graveyard(_)) && !input.graveyard_view;
    let mismatch = turn_changed
        || summon_open != duel.pending_summon.is_some()
        || (selection_open && duel.pending.is_none())
        || (matches!(input.mode, Mode::Move(_)) && duel.pending_summon.is_some());
    if mismatch {
        input.mode = Mode::Field;
        input.details = None;
        input.surrender = None;
        input.settings = None;
        input.hand_view = None;
        input.graveyard_view = false;
    }
    // An open summon (the hand refilled) is read in the hand menu, 0x43
    // (a resynchronised copy whose summon the input state missed).
    if duel.pending_summon.is_some() && !matches!(input.mode, Mode::Hand(_) | Mode::Confirm(_)) {
        input.mode = Mode::Hand(Default::default());
    }
}

/// The response the presentation gives the other seat's press: the input
/// layer's own (the replayed key on this copy) when it agrees in kind with
/// what the seat reported, else the reported kind.
pub fn remote_response(reported: PressResponse, replayed: Option<Response>) -> Response {
    match (reported, replayed) {
        (PressResponse::Commands, Some(r @ Response::Commands(_))) => r,
        (PressResponse::Commands, _) => Response::Commands(Vec::new()),
        (PressResponse::Moved, _) => Response::Moved,
        (PressResponse::Refused, _) => Response::Refused,
        (PressResponse::Ignored, _) => Response::Ignored,
        (PressResponse::Unsupported, _) => Response::Unsupported("the other seat's press (not modelled here)"),
    }
}

/// Where the other seat's change happened, for its cursor (the camera
/// follows the active side's cursor): the last summon, creation or move
/// destination, else its open summon cell.
pub fn remote_focus(events: &[DuelEvent], duel: &Duel) -> Option<roses_core::GridPos> {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            DuelEvent::Summoned { at, .. } | DuelEvent::Created { at, .. } => Some(*at),
            DuelEvent::Moved { to, .. } => Some(*to),
            _ => None,
        })
        .or(duel.pending_summon)
}

// ------------------------------------------------------------- pad bot

/// A computer player that plays through the pad, for headless online tests:
/// it picks intentions from its seat's copy (`roses_net::client::Bot`'s
/// choices: summon next to the leader, step toward the other leader, end
/// the turn) and turns them into the original's key sequences
/// (`roses_core::duel::ai::keys`), pressed one per update through the same
/// input layer a person uses.
#[derive(Default)]
pub struct PadBot {
    bot: roses_net::client::Bot,
    keys: VecDeque<u16>,
    round: Option<(u8, u8)>,
    /// This turn's summon was abandoned (nothing affordable): no new one.
    no_summon: Option<(u8, u8)>,
    /// Updates since the last plan (a stuck plan ends the turn).
    idle: u32,
}

impl PadBot {
    pub fn new() -> Self {
        Self::default()
    }

    /// The key for this update (0 = none). `can_press`: the client reads
    /// the pad now (its turn, nothing in flight, nothing animating).
    pub fn key(&mut self, duel: &Duel, input: &DuelUi, view: &PlayerView, revision: u64, can_press: bool, cards: &Catalogue) -> u16 {
        use roses_core::duel::ai::{self, pad, Action, Position};
        use roses_core::duel::DuelCommand;
        use roses_net::client::BotAction;
        if !can_press {
            return 0;
        }
        let turn = (duel.round, duel.active);
        if self.round != Some(turn) {
            self.round = Some(turn);
            self.keys.clear();
        }
        if let Some(k) = self.keys.pop_front() {
            self.idle = 0;
            return k;
        }
        self.idle += 1;
        if self.idle > 400 {
            self.idle = 0;
            return pad::START;
        }
        let side = duel.active;
        let s = usize::from(side & 1);
        // The hand menu after the refill: mark the strongest affordable
        // plain monster and summon it, or back out.
        if let (Mode::Hand(menu), Some(_)) = (&input.mode, duel.pending_summon) {
            let pick = duel.sides[s]
                .hand
                .iter()
                .enumerate()
                .filter_map(|(i, &id)| {
                    let c = roses_core::duel::CardData::card(cards, id)?;
                    (c.is_monster() && c.level <= duel.sides[s].stars)
                        .then_some((c.effects.iter().any(|&e| e), std::cmp::Reverse(c.attack), i))
                })
                .min()
                .map(|(_, _, i)| i);
            match pick {
                Some(i) => {
                    let key = if i >= menu.cursor { pad::RIGHT } else { pad::LEFT };
                    self.keys.extend(std::iter::repeat_n(key, i.abs_diff(menu.cursor)));
                    self.keys.extend([pad::UP, pad::CROSS, pad::CROSS]);
                    return self.keys.pop_front().unwrap_or(0);
                }
                None => {
                    self.no_summon = Some(turn);
                    return pad::CIRCLE;
                }
            }
        }
        // Close whatever the input layer has open before planning.
        if !matches!(input.mode, Mode::Field) || input.details.is_some() || input.surrender.is_some() || input.settings.is_some() || input.hand_view.is_some() {
            return pad::CIRCLE;
        }
        let cursor = input.cursors[s];
        loop {
            let action = match self.bot.next(view, revision, cards) {
                Some(BotAction::Command(c)) => c,
                Some(BotAction::Forfeit) | None => return pad::START,
            };
            let plan = match action {
                // Open the summon at the cell (Square at the leader, walk,
                // Cross); the card is chosen in the refilled hand menu.
                DuelCommand::OpenSummon { .. } if self.no_summon == Some(turn) => None,
                DuelCommand::OpenSummon { cell } => Some((Action::Summon { cell, hand: Vec::new() }, 2)),
                DuelCommand::Move { unit, path, .. } => path.last().map(|&target| {
                    (Action::Act { unit, target, reveal: false, position: Position::Keep }, 0)
                }),
                DuelCommand::EndTurn => return pad::START,
                DuelCommand::Pick { .. } => return pad::CIRCLE,
                _ => None,
            };
            let Some((action, drop_last)) = plan else { continue };
            let mut keys = ai::keys(duel, side, cursor, &action, cards);
            keys.truncate(keys.len().saturating_sub(drop_last));
            if keys.is_empty() {
                continue;
            }
            self.keys.extend(keys);
            self.idle = 0;
            return self.keys.pop_front().unwrap_or(0);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::duel_play::{DuelData, DuelPlay, FirstDuel, SideDeck};
    use roses_net::transport::memory_pair;
    use roses_server::serve::ServerConn;
    use roses_server::{MatchSetup, OnlineMatch};
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    /// What one Bevy client saw of a whole online match.
    struct ClientRun {
        seat: u8,
        updates: u64,
        presses: u64,
        outcome: Option<roses_core::duel::Outcome>,
        halted: Option<String>,
        mirror: Duel,
        refused_by_server: usize,
        animated: u64,
        /// The other seat's presses presented, its revisions presented
        /// without one, and updates showing its hand row / summon flight.
        remote_presses: u64,
        remote_unpressed: u64,
        remote_hand_rows: u64,
        remote_flights: u64,
        /// The details-pane slide (`DuelPlay::details_slide`, d3a) per update
        /// while this seat / the other seat had the turn, changes only.
        local_slides: Vec<u8>,
        remote_slides: Vec<u8>,
        /// Updates the pane stood at rest (d3a 64) on this seat's turn / the
        /// other seat's.
        rest: [u64; 2],
    }

    /// One details view on the seat's first turn: Triangle on the cursor's
    /// cell, 20 updates open, Circle (then the bot plays on).
    #[derive(Default)]
    struct DetailsTour {
        step: u8,
        open: u32,
    }

    impl DetailsTour {
        fn key(&mut self, play: &DuelPlay, seat: u8, can: bool) -> Option<u16> {
            use roses_core::duel::ai::pad;
            let input = &play.session.input;
            match self.step {
                0 if can && play.session.duel.active == seat && matches!(input.mode, Mode::Field) && input.details.is_none() && input.surrender.is_none() && input.settings.is_none() && input.hand_view.is_none() => {
                    self.step = 1;
                    Some(pad::TRIANGLE)
                }
                1 if input.details.is_some() => {
                    self.open += 1;
                    if self.open >= 20 && can {
                        self.step = 2;
                        Some(pad::CIRCLE)
                    } else {
                        Some(0)
                    }
                }
                // Triangle refused (a face-down enemy card): give up.
                1 => {
                    self.step = 3;
                    None
                }
                2 if input.details.is_some() || play.details_slide > 0 => Some(0),
                _ => None,
            }
        }
    }

    /// A DuelData around a catalogue without the disc files (the online
    /// client needs no planner tables and these tests draw no HUD).
    pub(crate) fn bare_data(cards: Catalogue, terrain: Vec<u8>) -> DuelData {
        let executable = vec![0u8; 0x1C_4D80 + 854 * 20 + 0x2000];
        let tables = roses_core::duel::ai::weevil::Tables::new(&cards.fusions, &executable).expect("zero tables");
        let side = |leader| SideDeck { deck_record: 0, leader, cards: Vec::new() };
        DuelData {
            catalogue: cards,
            names: Vec::new(),
            first_duel: FirstDuel {
                board_map: 0,
                terrain,
                first: 0,
                sides: [side(0), side(1)],
                computer_sides: Vec::new(),
                planner_tier: 0,
                leader_abilities: [vec![], vec![]],
                opponent: ONLINE_OPPONENT,
                leader_ranks: [0; 2],
                player_side: 0,
                custom_rules: None,
                options: None,
                card_ranks: Default::default(),
            },
            tables,
            executable,
            messages: Vec::new(),
            texsys: Vec::new(),
            picmini: Vec::new(),
            texeff: None,
            pictures: None,
        }
    }

    type HudDraw = Box<dyn FnMut(&DuelPlay)>;
    type HudFactory = Option<fn(&DuelPlay) -> HudDraw>;

    /// A Bevy online client driven by a pad-level bot, headless: connect,
    /// then one `DuelPlay::press` per update (the renderer's play_tick)
    /// until the match ends.
    fn bevy_client(
        data: DuelData,
        to_server: mpsc::Sender<ServerConn>,
        seat: u8,
        deck: DeckSubmission,
        tables: RulesTables,
        hud: HudFactory,
        max_updates: u64,
    ) -> ClientRun {
        let connector: Connector = Box::new(move || {
            let (client, server) = memory_pair::<ClientMessage, ServerMessage>();
            to_server
                .send(Box::new(server))
                .map_err(|_| std::io::Error::other("server gone"))?;
            Ok(Box::new(client) as ClientConn)
        });
        let cards = data.catalogue.clone();
        let link = OnlineLink::connect(connector, &cards, &tables, Join::Seat { seat: Some(seat) }, Some(deck), Duration::from_secs(60), |_| {})
            .expect("joined");
        assert_eq!(link.seat, seat);
        let mut play = DuelPlay::online(data, link).expect("online play");
        let mut draw = hud.map(|h| h(&play));
        let mut bot = PadBot::new();
        let mut camera = crate::duel_board::BoardCamera::snapped((3, 3));
        let (mut presses, mut animated, mut refused) = (0, 0, 0);
        let (mut remote_hand_rows, mut remote_flights) = (0, 0);
        let mut tour = DetailsTour::default();
        let (mut local_slides, mut remote_slides) = (vec![0u8], vec![0u8]);
        let mut rest = [0u64; 2];
        let mut last_status = None;
        for _ in 0..max_updates {
            let link = play.online.as_ref().expect("online");
            if link.ended() && link.state.revision == link.shown_revision && !play.animating() {
                break;
            }
            let can = !play.animating() && link.may_act();
            let view = link.state.view.clone().expect("a view");
            let revision = link.state.revision;
            let key = match tour.key(&play, seat, can) {
                Some(k) => k,
                None => bot.key(&play.session.duel, &play.session.input, &view, revision, can, &cards),
            };
            if key != 0 {
                presses += 1;
            }
            if play.animating() {
                animated += 1;
            }
            if play.session.duel.active != seat {
                remote_hand_rows += u64::from(play.hand_anim.is_some());
                remote_flights += u64::from(play.summon_flight.is_some());
            }
            play.press(key);
            let own_turn = play.session.duel.active == seat;
            if play.details_slide == 64 {
                rest[usize::from(!own_turn)] += 1;
            }
            let slides = if own_turn { &mut local_slides } else { &mut remote_slides };
            if slides.last() != Some(&play.details_slide) {
                slides.push(play.details_slide);
            }
            let cursor = play.cursor();
            crate::duel_play::drive_camera(&mut camera, &mut play, cursor, &|_| None);
            camera.tick();
            if let Some(d) = draw.as_mut() {
                d(&play);
            }
            if play.status != last_status {
                if std::env::var_os("ROSES_ONLINE_DEBUG").is_some() {
                    let l = play.online.as_ref().expect("online");
                    eprintln!(
                        "seat {seat} r{} active {} state {:#x} pending {:?}: {:?}",
                        l.shown_revision,
                        play.session.duel.active,
                        play.session.input.state(),
                        play.session.duel.pending_summon,
                        play.status
                    );
                }
                if play.status.as_deref().is_some_and(|s| s.contains("server refused")) {
                    refused += 1;
                }
                last_status = play.status.clone();
            }
            if play.halted.is_some() {
                break;
            }
            if !play.animating() && play.online.as_ref().is_some_and(|l| !l.may_act()) {
                // Waiting for the server or the other seat.
                std::thread::sleep(Duration::from_micros(200));
            }
        }
        ClientRun {
            seat,
            updates: play.frames,
            presses,
            outcome: play.session.duel.outcome,
            halted: play.halted.clone(),
            mirror: play.session.duel.clone(),
            refused_by_server: refused,
            animated,
            remote_presses: play.online.as_ref().map_or(0, |l| l.remote_presses),
            remote_unpressed: play.online.as_ref().map_or(0, |l| l.remote_unpressed),
            remote_hand_rows,
            remote_flights,
            local_slides,
            remote_slides,
            rest,
        }
    }

    /// A server thread with submitted decks and two headless Bevy clients.
    fn online_match(
        cards: Catalogue,
        setup: MatchSetup,
        decks: [DeckSubmission; 2],
        (tables, options): (RulesTables, MatchOptions),
        data: impl Fn() -> DuelData + Sync,
        hud: HudFactory,
    ) -> (OnlineMatch, [ClientRun; 2]) {
        let mut game = OnlineMatch::with_submitted_decks(
            "bevy-online-test",
            setup,
            cards,
            roses_net::OnlinePolicy { turn_limit: None, ..roses_net::OnlinePolicy::default() },
            [roses_server::secret_token(), roses_server::secret_token()],
        )
        .unwrap()
        .with_rules(tables.clone(), options)
        .unwrap();
        let (tx, mut rx) = mpsc::channel::<ServerConn>();
        let stop = AtomicBool::new(false);
        let runs = std::thread::scope(|scope| {
            let server = scope.spawn(|| {
                // The headless clients run their updates far faster than
                // 25 Hz: no message budget here.
                roses_server::serve::serve_with_limits(&mut rx, &mut game, &stop, roses_server::serve::Limits::UNLIMITED, |_| {})
                    .unwrap();
            });
            let data = &data;
            let clients = [0u8, 1].map(|seat| {
                let tx = tx.clone();
                let deck = decks[usize::from(seat)].clone();
                let tables = tables.clone();
                scope.spawn(move || bevy_client(data(), tx, seat, deck, tables, hud, 400_000))
            });
            let runs = clients.map(|c| c.join().unwrap());
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
            server.join().unwrap();
            runs
        });
        (game, runs)
    }

    /// The other seat's first presses can reach this seat in the same read
    /// as the opening snapshot, while `connect` is still running: they are
    /// kept for the screen (seat 1 used to take them for its own and drop
    /// them, so the first details pane never slid in).
    #[test]
    fn presses_read_with_the_opening_snapshot_are_kept() {
        let cards = roses_net::synthetic::catalogue();
        let setup = (roses_server::hub::HubData::synthetic(false).setup)(1);
        let tokens = [roses_server::secret_token(), roses_server::secret_token()];
        let mut game = OnlineMatch::with_submitted_decks("early", setup, cards.clone(), roses_net::OnlinePolicy::default(), tokens).unwrap();
        let tables = RulesTables::empty();
        let compat = Compat::current(data_fingerprint(&cards, &tables));
        let decks = roses_net::synthetic::submissions();
        let mut to_seat_1 = Vec::new();
        for seat in 0..2u8 {
            let join = Join::Seat { seat: Some(seat) };
            let (_, out) = game.hello_streaming(&compat, &join, Some(&decks[usize::from(seat)]), true, 0).unwrap();
            to_seat_1.extend(out.into_iter().filter(|(s, _)| *s == 1).map(|(_, m)| m));
        }
        assert_eq!(game.duel().active, 0);
        let revision = game.revision();
        let input = roses_core::duel::ui::DuelUi::new(game.duel());
        let press = roses_net::Press { base_revision: revision, through_revision: revision, key: 0x10, response: roses_net::PressResponse::Moved, input, gap: 0 };
        let out = game.handle(0, ClientMessage::Presence { press }, 0);
        to_seat_1.extend(out.into_iter().filter(|(s, _)| *s == 1).map(|(_, m)| m));
        let keep = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let held = keep.clone();
        let connector: Connector = Box::new(move || {
            let (client, mut server) = memory_pair::<ClientMessage, ServerMessage>();
            for m in &to_seat_1 {
                server.send(m).unwrap();
            }
            held.lock().unwrap().push(server);
            Ok(Box::new(client) as ClientConn)
        });
        let link = OnlineLink::connect(connector, &cards, &tables, Join::Seat { seat: Some(1) }, Some(decks[1].clone()), Duration::from_secs(5), |_| {}).unwrap();
        assert_eq!(link.seat, 1);
        assert_eq!(link.queue.iter().filter(|i| matches!(i, Item::Press(_))).count(), 1);
    }

    /// The server restarted (the match is gone): a seat's resume is refused
    /// and the duel stops, where it used to retry every second for ever; a
    /// server that never comes back ends the same way.
    #[test]
    fn a_seat_whose_match_is_gone_stops_reconnecting() {
        let cards = roses_net::synthetic::catalogue();
        let setup = (roses_server::hub::HubData::synthetic(false).setup)(1);
        let tokens = [roses_server::secret_token(), roses_server::secret_token()];
        let mut game = OnlineMatch::with_submitted_decks("gone", setup, cards.clone(), roses_net::OnlinePolicy::default(), tokens).unwrap();
        let (tx, mut rx) = mpsc::channel::<ServerConn>();
        let stop = AtomicBool::new(false);
        let connector = |tx: mpsc::Sender<ServerConn>| -> Connector {
            Box::new(move || {
                let (client, server) = memory_pair::<ClientMessage, ServerMessage>();
                tx.send(Box::new(server)).map_err(|_| std::io::Error::other("server gone"))?;
                Ok(Box::new(client) as ClientConn)
            })
        };
        let decks = roses_net::synthetic::submissions();
        let [mut one, mut two] = std::thread::scope(|scope| {
            let server = scope.spawn(|| {
                roses_server::serve::serve_with_limits(&mut rx, &mut game, &stop, roses_server::serve::Limits::UNLIMITED, |_| {}).unwrap();
            });
            let links = [0u8, 1].map(|seat| {
                let (c, cards, deck) = (connector(tx.clone()), cards.clone(), decks[usize::from(seat)].clone());
                scope.spawn(move || {
                    OnlineLink::connect(c, &cards, &RulesTables::empty(), Join::Seat { seat: Some(seat) }, Some(deck), Duration::from_secs(30), |_| {}).unwrap()
                })
            });
            let links = links.map(|l| l.join().unwrap());
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
            server.join().unwrap();
            links
        });
        let start = Instant::now();
        let mut conn = loop {
            one.pump();
            if let Ok(conn) = rx.try_recv() {
                break conn;
            }
            assert!(start.elapsed() < Duration::from_secs(20), "the seat did not try to resume");
            std::thread::sleep(Duration::from_millis(2));
        };
        loop {
            match conn.poll() {
                Received::Message(ClientMessage::Hello { .. }) => break,
                Received::Closed => panic!("no hello"),
                _ => std::thread::sleep(Duration::from_millis(2)),
            }
        }
        // A server with no room yet: still trying.
        conn.send(&ServerMessage::Refused { reason: Refusal::ServerBusy }).unwrap();
        one.pump();
        assert_eq!(one.sent_away, None);
        conn.send(&ServerMessage::Refused { reason: Refusal::BadToken }).unwrap();
        one.pump();
        assert_eq!(one.sent_away, Some(Refusal::BadToken.plain()));
        // Nothing answers any more: the other seat gives up after the wait.
        drop(rx);
        two.give_up_after = Duration::ZERO;
        two.pump();
        two.pump();
        assert_eq!(two.sent_away.as_deref(), Some(LOST_SERVER));
    }

    fn check(game: &OnlineMatch, runs: &[ClientRun; 2], cards: &Catalogue, tables: &RulesTables) {
        let (outcome, cause) = game.ended().expect("the match ended");
        eprintln!("ended {outcome:?} ({cause:?}) at revision {}: {:?}", game.revision(), game.replay().entries.iter().rev().take(6).map(|e| format!("{:?}", e.action)).collect::<Vec<_>>());
        for run in runs {
            eprintln!(
                "seat {}: {} updates, {} presses, {} animated; the other seat: {} presses presented, {} revisions without one, {} hand-row and {} flight updates",
                run.seat, run.updates, run.presses, run.animated, run.remote_presses, run.remote_unpressed, run.remote_hand_rows, run.remote_flights
            );
            assert_eq!(run.halted, None, "seat {}", run.seat);
            assert_eq!(run.outcome, Some(outcome), "seat {}", run.seat);
            // The client's copy is exactly what its view allows: the
            // server's final view rebuilt.
            assert_eq!(
                roses_net::player_view(&run.mirror, run.seat),
                roses_net::player_view(game.duel(), run.seat),
                "seat {}",
                run.seat
            );
            assert!(run.presses > 20, "seat {} pressed {} keys", run.seat, run.presses);
            assert!(run.animated > 0, "seat {} presented nothing", run.seat);
            // The other seat's turns were presented through its streamed
            // presses: its cursor and menus, its hand row and summon flight.
            assert!(run.remote_presses > 10, "seat {}: {} remote presses", run.seat, run.remote_presses);
            assert_eq!(run.remote_unpressed, 0, "seat {}: a revision of the other seat lost its press", run.seat);
            assert!(run.remote_hand_rows > 0 && run.remote_flights > 0, "seat {}: no remote hand row / flight", run.seat);
            // Each seat opened a details view once on its first turn: the
            // other seat's screen slides the pane in (d3a 8..64 by 8) and out
            // (56..0) from the streamed Triangle / Circle, as the seat's own.
            let slide: Vec<u8> = (0..=64).step_by(8).chain((0..64).step_by(8).rev()).collect();
            let has = |seq: &[u8]| seq.windows(slide.len()).any(|w| w == slide.as_slice());
            eprintln!("seat {}: details slide own {:?}, the other seat's {:?}", run.seat, &run.local_slides[..run.local_slides.len().min(20)], &run.remote_slides[..run.remote_slides.len().min(20)]);
            assert!(has(&run.local_slides), "seat {}: own slide {:?}", run.seat, run.local_slides);
            assert!(has(&run.remote_slides), "seat {}: the other seat's slide {:?}", run.seat, run.remote_slides);
        }
        // The pane rests as long on the watching screen as on the pressing one
        // (the streamed press gaps).
        for (run, other) in [(&runs[0], &runs[1]), (&runs[1], &runs[0])] {
            eprintln!("seat {}: pane at rest {} updates, on the other screen {}", run.seat, run.rest[0], other.rest[1]);
            assert_eq!(run.rest[0], other.rest[1], "seat {}'s details rest on the other screen", run.seat);
        }
        // Both seats summoned and moved through their pads.
        let commands: Vec<_> = game
            .replay()
            .entries
            .iter()
            .filter_map(|e| match &e.action {
                roses_server::replay::ReplayAction::Command { side, command } => Some((*side, command.clone())),
                _ => None,
            })
            .collect();
        for side in 0..2u8 {
            assert!(
                commands.iter().any(|(s, c)| *s == side && matches!(c, roses_core::duel::DuelCommand::ChooseSummon { .. })),
                "side {side}"
            );
            assert!(
                commands.iter().any(|(s, c)| *s == side && matches!(c, roses_core::duel::DuelCommand::Move { .. })),
                "side {side}"
            );
        }
        assert_eq!(roses_server::replay(game.replay(), cards, tables).unwrap(), *game.duel());
        eprintln!(
            "online match: {outcome:?} at revision {}; seat 0 {} updates / {} presses / {} refused, seat 1 {} / {} / {}",
            game.revision(),
            runs[0].updates,
            runs[0].presses,
            runs[0].refused_by_server,
            runs[1].updates,
            runs[1].presses,
            runs[1].refused_by_server
        );
    }

    /// Through a whole match under Card Display HIDE, on both seats' turns:
    /// the HUD names a seat's own hand and own face-down cards, and never
    /// the other seat's face-down cards or hand.
    #[test]
    fn a_seat_reads_its_own_cards_but_never_the_other_seats_hidden_ones() {
        use roses_client::duel_hud::{from_duel, ui_from_input, HudUpdate};
        use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
        // Own face-down, enemy face-down, own hand, enemy hand; own turn and
        // the other seat's.
        static SEEN: [[AtomicU64; 2]; 4] = [const { [const { AtomicU64::new(0) }; 2] }; 4];
        let hud: HudFactory = Some(|_: &DuelPlay| {
            Box::new(|p: &DuelPlay| {
                let seat = p.online.as_ref().unwrap().seat;
                let duel = &p.session.duel;
                let previous = HudUpdate { d3c: duel.active, ..HudUpdate::default() };
                let mut ui = ui_from_input(duel, &p.session.input, &previous, 0);
                ui.hand_records = true;
                ui.extra.own_side = p.hud_own_side();
                let st = from_duel(duel, &ui, &p.data.catalogue, &p.hud_context(), &|_, _| 0, &previous);
                let turn = usize::from(duel.active != seat);
                let (field, hand) = st.units.split_at(duel.units.len());
                for u in field.iter().filter(|u| u.slot > 0 && u.flags & 0x14 == 0) {
                    let own = u.controller == seat;
                    assert_eq!(u.face_down_hidden, !own, "seat {seat}, card {} on side {}, active {}", u.card, u.side, duel.active);
                    SEEN[usize::from(!own)][turn].fetch_add(1, Relaxed);
                }
                for u in hand {
                    assert_eq!(u.face_down_hidden, duel.active != seat, "seat {seat}, hand of side {}", duel.active);
                    SEEN[2 + turn][turn].fetch_add(1, Relaxed);
                }
            })
        });
        let setup = MatchSetup {
            seed: 99,
            terrain: roses_net::synthetic::board(),
            leaders: roses_net::synthetic::leaders(),
            deck_sources: roses_net::synthetic::decks(),
            first: 0,
            leader_abilities: [vec![], vec![]],
            custom_rules: None,
            leader_ranks: [0; 2],
            destiny_rows: None,
        };
        let data = || bare_data(roses_net::synthetic::catalogue(), roses_net::synthetic::board());
        let rules = (RulesTables::empty(), MatchOptions::default());
        online_match(roses_net::synthetic::catalogue(), setup, roses_net::synthetic::submissions(), rules, data, hud);
        let seen = SEEN.each_ref().map(|s| s.each_ref().map(|n| n.load(Relaxed)));
        eprintln!("own down, enemy down, own hand, enemy hand: {seen:?}");
        assert!(seen[2][0] > 0 && seen[3][1] > 0, "{seen:?}");
        assert!(seen[0].iter().chain(&seen[1]).all(|&n| n > 0), "{seen:?}");
    }

    /// Two headless Bevy clients (pad presses through `DuelPlay`, the input
    /// layer on the view-built copy, intentions to the server) play a whole
    /// match with submitted decks against the authoritative server.
    #[test]
    fn two_bevy_clients_play_an_online_match_through_their_pads() {
        let cards = roses_net::synthetic::catalogue();
        let setup = MatchSetup {
            seed: 99,
            terrain: roses_net::synthetic::board(),
            leaders: roses_net::synthetic::leaders(),
            deck_sources: roses_net::synthetic::decks(),
            first: 0,
            leader_abilities: [vec![], vec![]],
            custom_rules: None,
            leader_ranks: [0; 2],
            destiny_rows: None,
        };
        let decks = roses_net::synthetic::submissions();
        let data = || bare_data(roses_net::synthetic::catalogue(), roses_net::synthetic::board());
        let rules = (RulesTables::empty(), MatchOptions::default());
        let (game, runs) = online_match(cards.clone(), setup, decks, rules.clone(), data, None);
        check(&game, &runs, &cards, &rules.0);
    }

    /// The same with the user's own import: real cards and effects, the
    /// first duel's board and decks submitted per seat, and the original
    /// HUD drawn by both clients every update (`ROSES_DUEL_IMPORT`).
    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_two_bevy_clients_play_the_imported_decks_online() {
        let Some(dir) = std::env::var_os("ROSES_DUEL_IMPORT").map(std::path::PathBuf::from) else { return };
        let import = roses_net::data::load_duel_import(&dir).unwrap();
        let cards = import.catalogue.clone();
        let decks = [0, 1].map(|s| roses_net::deck::from_import_side(&import.first_duel.sides[s]));
        // Seed 3: the server defaults (Leader Ability USE, Card Display
        // HIDE); seed 4: the original Custom Duel defaults (USE, OPEN).
        for (seed, open) in [(3u64, false), (4, true)] {
            let rules = (import.tables.clone(), MatchOptions { leader_ability: [true; 2], card_display_open: [open; 2] });
            let setup = MatchSetup::from_import(&import.first_duel, seed);
            let data = || crate::duel_play::load(&dir).unwrap();
            let hud: HudFactory = Some(|play: &DuelPlay| {
                let mut hud = crate::duel_play::DuelHud::new(play).expect("HUD");
                Box::new(move |p: &DuelPlay| {
                    hud.update(p);
                    hud.render();
                })
            });
            let (game, runs) = online_match(cards.clone(), setup, decks.clone(), rules.clone(), data, hud);
            check(&game, &runs, &cards, &rules.0);
            for run in &runs {
                // Two pad controllers: the result is the 2-player one
                // (1P WIN / 2P WIN / DRAW banners).
                assert!(run.mirror.controllers.iter().all(|&c| c >= roses_core::duel::PAD_CONTROLLER));
            }
        }
    }

    /// Online decks with the user's own import: a custom deck built with
    /// the game's deck editor model on the full-pool block (the pad words
    /// the end-to-end script gives the builder: 40 cards from the top of
    /// the trunk list, Start for the leader, Circle) against Pegasus
    /// Crawford's static deck, both submitted at join and played by two
    /// headless Bevy clients with the HUD drawn (`ROSES_DUEL_IMPORT`).
    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_a_built_custom_deck_plays_pegasus_deck_online() {
        use roses_core::deck_editor::{DeckEditor, EditorEnv, PadInput, StaticCards};
        use roses_net::custom_decks::{editor_block, from_editor_block, CustomDeck};
        let Some(dir) = std::env::var_os("ROSES_DUEL_IMPORT").map(std::path::PathBuf::from) else { return };
        let import = roses_net::data::load_duel_import(&dir).unwrap();
        let cards = import.catalogue.clone();
        let duel_dir = if dir.join("SLES_515.79").is_file() { dir.clone() } else { dir.join("duel") };
        let exe = std::fs::read(duel_dir.join("SLES_515.79")).unwrap();
        let statics = StaticCards::from_executable(&exe, 0).unwrap();
        let mut deck = CustomDeck::new("Dragons");
        deck.leader_rank = 5;
        let mut block = editor_block(&deck, &statics);
        let (mut counter, mut rng) = (0u8, 7u64);
        let mut editor = {
            let mut env = EditorEnv { block: &mut block, duel_counter: &mut counter, cards: &statics, rng: &mut rng, graphics_busy: false };
            DeckEditor::new(0, &mut env)
        };
        let mut words: Vec<u16> = (0..40).flat_map(|_| [0x40, 0, 0x4000, 0]).collect();
        words.extend([0x800, 0]);
        words.extend([0; 12]);
        words.extend([0x20, 0]);
        for w in words {
            let mut env = EditorEnv { block: &mut block, duel_counter: &mut counter, cards: &statics, rng: &mut rng, graphics_busy: false };
            editor.step(PadInput { word: w, sticks: None }, &mut env);
        }
        assert!(editor.finished(), "the builder left with a valid deck");
        let built = from_editor_block(&block, &deck);
        assert_eq!(built.problem(), None, "{built:?}");
        let custom = built.submission().unwrap();
        assert_eq!(custom.leader_rank, 5);
        assert_eq!(roses_net::deck::check_deck(&custom, &cards), Ok(()));
        let pegasus = roses_net::deck::load_opponent_decks(&duel_dir).unwrap().into_iter().find(|o| o.name == "Pegasus Crawford").unwrap();
        assert_eq!((pegasus.deck.leader, pegasus.deck.leader_rank), (61, 9));
        let decks = [custom, pegasus.deck];
        let rules = (import.tables.clone(), MatchOptions::default());
        let setup = MatchSetup::from_import(&import.first_duel, 21);
        let data = || crate::duel_play::load(&dir).unwrap();
        let hud: HudFactory = Some(|play: &DuelPlay| {
            let mut hud = crate::duel_play::DuelHud::new(play).expect("HUD");
            Box::new(move |p: &DuelPlay| {
                hud.update(p);
                hud.render();
            })
        });
        let (game, runs) = online_match(cards.clone(), setup, decks, rules.clone(), data, hud);
        check(&game, &runs, &cards, &rules.0);
    }
}
