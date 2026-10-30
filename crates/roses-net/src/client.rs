//! Client-side bookkeeping: sequence numbers, the last known revision and
//! view, unacknowledged intentions (resent after a reconnect; the server
//! never executes a `seq` twice), and a simple computer player that picks
//! intentions from the seat's own view only.

use crate::filter::PlayerView;
use crate::protocol::{Cause, ClientMessage, Decision, MatchOptions, MatchStatus, Refusal, ServerMessage};
use roses_core::duel::{CardData, DuelCommand, Outcome, Pick, LEADER_CELLS};
use roses_core::GridPos;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub struct ClientState {
    pub seat: Option<u8>,
    pub match_id: Option<String>,
    pub token: Option<String>,
    pub revision: u64,
    pub view: Option<PlayerView>,
    pub status: Option<MatchStatus>,
    pub ended: Option<(Outcome, Cause)>,
    pub refused: Option<Refusal>,
    /// Intentions sent but not yet decided, by seq.
    pub unacked: BTreeMap<u64, ClientMessage>,
    /// The latest decision received (seq, decision).
    pub last_decision: Option<(u64, Decision)>,
    /// The match's Custom Duel switches (from the welcome).
    pub options: Option<MatchOptions>,
    /// Which seats stream their presses (from the last snapshot).
    pub presence: Vec<bool>,
    /// A spectator's connection ([`ServerMessage::Watching`]): no seat, no
    /// intentions.
    pub spectator: bool,
    /// The players' names by seat (a spectator's welcome).
    pub names: Option<[String; 2]>,
    /// Spectators watching the match (the last [`ServerMessage::Watchers`]).
    pub watchers: u16,
    next_seq: u64,
}

impl ClientState {
    pub fn new() -> Self {
        Self {
            next_seq: 1,
            ..Self::default()
        }
    }

    fn take_seq(&mut self) -> u64 {
        let seq = self.next_seq.max(1);
        self.next_seq = seq + 1;
        seq
    }

    /// A sequenced intention based on the current revision.
    pub fn command(&mut self, command: DuelCommand) -> ClientMessage {
        let seq = self.take_seq();
        let msg = ClientMessage::Command {
            seq,
            base_revision: self.revision,
            command,
        };
        self.unacked.insert(seq, msg.clone());
        msg
    }

    pub fn forfeit(&mut self) -> ClientMessage {
        let seq = self.take_seq();
        let msg = ClientMessage::Forfeit { seq };
        self.unacked.insert(seq, msg.clone());
        msg
    }

    /// Undecided intentions, oldest first (to resend after reconnecting).
    pub fn resend(&self) -> Vec<ClientMessage> {
        self.unacked.values().cloned().collect()
    }

    pub fn apply(&mut self, msg: &ServerMessage) {
        match msg {
            ServerMessage::Welcome {
                match_id,
                seat,
                resume_token,
                options,
                ..
            } => {
                self.options = Some(*options);
                self.match_id = Some(match_id.clone());
                self.seat = Some(*seat);
                self.token = Some(resume_token.clone());
            }
            ServerMessage::Refused { reason } => self.refused = Some(reason.clone()),
            ServerMessage::Snapshot { snapshot } => {
                self.revision = snapshot.revision;
                self.view = Some(snapshot.view.clone());
                self.status = Some(snapshot.status);
                self.presence = snapshot.presence.clone();
                self.next_seq = self.next_seq.max(snapshot.last_seq + 1);
                if let MatchStatus::Ended { outcome, cause } = snapshot.status {
                    self.ended = Some((outcome, cause));
                }
            }
            ServerMessage::Accepted { seq, revision } => {
                self.unacked.remove(seq);
                self.last_decision = Some((
                    *seq,
                    Decision::Accepted {
                        revision: *revision,
                    },
                ));
            }
            ServerMessage::Rejected { seq, reason, .. } => {
                self.unacked.remove(seq);
                self.last_decision = Some((
                    *seq,
                    Decision::Rejected {
                        reason: reason.clone(),
                    },
                ));
            }
            ServerMessage::Duplicate { seq, decision } => {
                self.unacked.remove(seq);
                self.last_decision = Some((*seq, decision.clone()));
            }
            ServerMessage::Update { revision, view, .. } => {
                self.revision = *revision;
                self.view = Some(view.clone());
            }
            ServerMessage::Paused { absent, .. } => {
                self.status = Some(MatchStatus::Paused { absent: *absent })
            }
            ServerMessage::Resumed => self.status = Some(MatchStatus::Running),
            ServerMessage::Ended { outcome, cause, .. } => {
                self.ended = Some((*outcome, *cause));
                self.status = Some(MatchStatus::Ended {
                    outcome: *outcome,
                    cause: *cause,
                });
            }
            ServerMessage::LobbyRefused { reason } => {
                use crate::lobby::LobbyRefusal;
                self.refused = Some(match reason {
                    LobbyRefusal::Incompatible { detail } => Refusal::Incompatible { detail: detail.clone() },
                    LobbyRefusal::ExpectedHello | LobbyRefusal::LobbyVersion { .. } => Refusal::ExpectedHello,
                    LobbyRefusal::ServerFull => Refusal::ServerBusy,
                });
            }
            ServerMessage::Watching { match_id, names, options, .. } => {
                self.spectator = true;
                self.match_id = Some(match_id.clone());
                self.names = Some(names.clone());
                self.options = Some(*options);
            }
            ServerMessage::Watchers { count } => self.watchers = *count,
            ServerMessage::ProtocolError { .. } | ServerMessage::Presence { .. } => {}
        }
    }

    /// Whether this seat may act now (running, its turn, not ended).
    pub fn my_turn(&self) -> bool {
        !self.spectator
            && matches!(self.status, Some(MatchStatus::Running))
            && self.ended.is_none()
            && self
                .view
                .as_ref()
                .zip(self.seat)
                .is_some_and(|(v, s)| v.board.active == s && v.board.outcome.is_none())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BotAction {
    Command(DuelCommand),
    /// Nothing it knows how to do was accepted: concede.
    Forfeit,
}

/// A simple computer player using only its seat's view and its own card
/// data: summon the strongest affordable plain monster next to its leader,
/// step every unit toward the opposing leader, end the turn. Refused
/// intentions are not repeated at the same revision.
#[derive(Clone, Debug, Default)]
pub struct Bot {
    tried: BTreeSet<String>,
    tried_revision: u64,
    gave_up_summon: Option<(u8, u8)>,
}

impl Bot {
    pub fn new() -> Self {
        Self::default()
    }

    /// The next intention for `view` at `revision`, or `None` when it is
    /// not this seat's turn.
    pub fn next(
        &mut self,
        view: &PlayerView,
        revision: u64,
        cards: &impl CardData,
    ) -> Option<BotAction> {
        let seat = view.seat;
        let b = &view.board;
        if b.outcome.is_some() || b.active != seat {
            return None;
        }
        if revision != self.tried_revision {
            self.tried.clear();
            self.tried_revision = revision;
        }
        let turn = (b.round, seat);
        for command in self.candidates(view, cards) {
            let key = serde_json::to_string(&command).expect("serialisable");
            if self.tried.insert(key) {
                if command == DuelCommand::CancelSummon {
                    self.gave_up_summon = Some(turn);
                }
                return Some(BotAction::Command(command));
            }
        }
        Some(BotAction::Forfeit)
    }

    fn candidates(&self, view: &PlayerView, cards: &impl CardData) -> Vec<DuelCommand> {
        let seat = view.seat;
        let b = &view.board;
        let s = usize::from(seat);
        let mut out = Vec::new();
        if view.pending.is_some() {
            out.push(DuelCommand::Pick { pick: Pick::Cancel });
            return out;
        }
        if view.pending_summon.is_some() {
            let mut options: Vec<(usize, u16, bool)> = b
                .hand
                .iter()
                .enumerate()
                .filter_map(|(i, &id)| {
                    let c = cards.card(id)?;
                    (c.is_monster() && c.level <= b.stars[s])
                        .then_some((i, c.attack, c.effects.iter().any(|&e| e)))
                })
                .collect();
            // Plain monsters first, strongest first.
            options.sort_by_key(|&(i, attack, effect)| (effect, std::cmp::Reverse(attack), i));
            out.extend(
                options
                    .into_iter()
                    .map(|(i, _, _)| DuelCommand::ChooseSummon { hand: vec![i] }),
            );
            out.push(DuelCommand::CancelSummon);
            return out;
        }
        let occupied = |p: GridPos| b.units.iter().any(|u| u.position == p);
        // N-player duels (a port addition): the own leader where it stands,
        // and the nearest living enemy leader as the target.
        let (leader, target, forward) = match &view.multi {
            None => (LEADER_CELLS[s], LEADER_CELLS[s ^ 1], (0, if seat == 0 { -1 } else { 1 })),
            Some(m) => {
                let cell = |side: u8| b.units.iter().find(|u| u.side == side && u.slot == 0).map(|u| u.position);
                let Some(own) = cell(seat) else {
                    out.push(DuelCommand::EndTurn);
                    return out;
                };
                let enemy = (0..b.lp.len() as u8)
                    .filter(|&o| {
                        o != seat
                            && m.out.get(usize::from(o)).is_some_and(Option::is_none)
                            && m.teams.get(usize::from(o)) != m.teams.get(s)
                    })
                    .filter_map(cell)
                    .min_by_key(|p| p.column.abs_diff(own.column) + p.row.abs_diff(own.row))
                    .unwrap_or(own);
                let step = |a: u8, b: u8| (i16::from(b) - i16::from(a)).signum() as i8;
                let (dc, dr) = (step(own.column, enemy.column), step(own.row, enemy.row));
                (own, enemy, if dc.abs() >= dr.abs() && dc != 0 { (dc, 0) } else { (0, dr) })
            }
        };
        if view.can_summon[s]
            && self.gave_up_summon != Some((b.round, seat))
            && b.hand.len() + b.deck_counts[s] > 0
        {
            // Ahead, ahead to either side, then either side.
            let (fc, fr) = forward;
            let sides = if fc == 0 { [(-1, 0), (1, 0)] } else { [(0, -1), (0, 1)] };
            let cells = [
                (fc, fr),
                (fc + sides[0].0, fr + sides[0].1),
                (fc + sides[1].0, fr + sides[1].1),
                sides[0],
                sides[1],
            ];
            for (dc, dr) in cells {
                if let Some(cell) = offset(leader, dc, dr) {
                    if !occupied(cell) {
                        out.push(DuelCommand::OpenSummon { cell });
                    }
                }
            }
        }
        for u in &b.units {
            if u.side != seat || u.slot == 0 || u.acted || u.spellbound {
                continue;
            }
            if !u
                .card
                .and_then(|c| cards.card(c))
                .is_some_and(|c| c.is_monster())
            {
                continue;
            }
            let dc = (target.column as i8 - u.position.column as i8).signum();
            let dr = (target.row as i8 - u.position.row as i8).signum();
            let mut steps = Vec::new();
            if dr != 0 {
                steps.push((0, dr));
            }
            if dc != 0 {
                steps.push((dc, 0));
            }
            for (c, r) in steps {
                if let Some(cell) = offset(u.position, c, r) {
                    let own = b.units.iter().any(|o| o.position == cell && o.side == seat);
                    if !own {
                        out.push(DuelCommand::Move {
                            unit: u.id,
                            path: vec![cell],
                            face_up: false,
                            defence: None,
                        });
                    }
                }
            }
        }
        out.push(DuelCommand::EndTurn);
        out
    }
}

fn offset(p: GridPos, dc: i8, dr: i8) -> Option<GridPos> {
    let column = p.column as i8 + dc;
    let row = p.row as i8 + dr;
    let cell = GridPos {
        column: u8::try_from(column).ok()?,
        row: u8::try_from(row).ok()?,
    };
    cell.index().map(|_| cell)
}
