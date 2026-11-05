//! Port addition, not in the 2003 game (docs/ADDITIONS.md): the 4-player
//! modes, a free-for-all of three or four players and 2v2, played on the
//! original 7 × 7 board. Every rule here is invented
//! (the project notes §2):
//!
//! - leaders at the middle of each edge (S (3,6) side 0, N (3,0) side 1,
//!   E (6,3) side 2, W (0,3) side 3), summon areas unchanged;
//! - turns round the table (0, 2, 1, 3; in 2v2 the teams alternate), a
//!   round ends when play returns to the first player;
//! - "the opponent" of an effect: inside a battle the battle partner's
//!   controller; outside a battle the player's chosen target
//!   ([`DuelCommand::Target`], automatic while one enemy is left); mode
//!   masks ("other side") mean every enemy, so a team-mate is spared;
//!   full-range trap searches look through the enemies in turn order;
//! - a player is knocked out at 0 LP, with a trapped leader at their turn
//!   end, on surrender or forfeit; their turns are skipped and, by the
//!   room setting [`MultiRules::cards_leave`], their cards leave the board
//!   (no effects, no experience; stolen units go back to their owners) or
//!   stay as obstacles. The match ends when one player (or team) is left,
//!   or at the round limit by LP.
//!
//! None of this runs in a 2-player duel: [`Duel::multi`] is `None` there,
//! every helper below then answers exactly what the original's `side ^ 1`
//! did, and the state serialises as before. The state and the constructor
//! exist only with the `online` feature.

use super::{CardData, Duel, DuelError, DuelEvent, EndReason, Result, MAX_LP, START_LP};
use crate::GridPos;
use serde::{Deserialize, Serialize};

/// Leader cells by side: S, N (the 2-player cells), E, W.
pub const MULTI_LEADER_CELLS: [GridPos; 4] = [
    GridPos { column: 3, row: 6 },
    GridPos { column: 3, row: 0 },
    GridPos { column: 6, row: 3 },
    GridPos { column: 0, row: 3 },
];
/// Turn order round the table (S, E, N, W).
pub const MULTI_TURN_ORDER: [u8; 4] = [0, 2, 1, 3];
/// The round limit of an online N-player match (the 2-player limit is 100).
pub const MULTI_ROUND_LIMIT: u8 = 40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiMode {
    /// Every player for themselves (3 or 4 players).
    #[default]
    FreeForAll,
    /// 2v2: sides 0 and 1 (S + N) against 2 and 3 (E + W).
    Teams,
}

/// The room settings of an N-player match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiRules {
    pub mode: MultiMode,
    /// A knocked-out player's cards leave the board (default); false: they
    /// stay as obstacles (they never act again, and can still be attacked).
    #[serde(default = "yes")]
    pub cards_leave: bool,
    /// 2v2: team-mates share one LP total (default: separate LP).
    #[serde(default)]
    pub shared_lp: bool,
    /// The last round; then LP decides.
    #[serde(default = "round_limit")]
    pub round_limit: u8,
    /// Every player's starting LP (a team's, with shared LP).
    #[serde(default = "start_lp")]
    pub start_lp: u16,
}

fn yes() -> bool {
    true
}
fn round_limit() -> u8 {
    MULTI_ROUND_LIMIT
}
fn start_lp() -> u16 {
    START_LP
}

impl Default for MultiRules {
    fn default() -> Self {
        Self {
            mode: MultiMode::FreeForAll,
            cards_leave: true,
            shared_lp: false,
            round_limit: MULTI_ROUND_LIMIT,
            start_lp: START_LP,
        }
    }
}

/// The campaign deck build ([`super::build_campaign_decks`], the duel
/// init loop 0x1AE7B0) for any number of sides: for i in 0..40, side by
/// side, each card is inserted at a random free slot.
pub fn build_decks(rng: &mut u64, sources: &[&[u16]]) -> Vec<Vec<u16>> {
    use super::{next_rand31, CARD_COUNT, DECK_SIZE, REJECTED_DECK_CARD};
    let mut decks = vec![[None::<u16>; DECK_SIZE]; sources.len()];
    for i in 0..DECK_SIZE {
        for (side, source) in sources.iter().enumerate() {
            let Some(&card) = source.get(i) else {
                continue;
            };
            if card == REJECTED_DECK_CARD || usize::from(card) >= usize::from(CARD_COUNT) {
                continue;
            }
            loop {
                let r = next_rand31(rng);
                let idx = (40.0_f32 * (r as f32 / 2_147_483_648.0_f32)) as usize;
                if idx < DECK_SIZE && decks[side][idx].is_none() {
                    decks[side][idx] = Some(card);
                    break;
                }
            }
        }
    }
    decks.into_iter().map(|d| d.into_iter().flatten().collect()).collect()
}

/// How and when a player was knocked out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knockout {
    pub round: u8,
    pub reason: EndReason,
    /// Their placing: 4 for the first of four players out, 2 for the last.
    pub place: u8,
}

/// The N-player state of a [`Duel`] (all of it public).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Multi {
    pub rules: MultiRules,
    /// Team by side (FFA: the side itself).
    pub teams: Vec<u8>,
    /// Turn order (sides).
    pub order: Vec<u8>,
    /// Knocked-out sides.
    pub out: Vec<Option<Knockout>>,
    /// Each side's chosen target for "the opponent" outside a battle.
    pub targets: Vec<u8>,
    /// The battle being fought (attacker's and defender's controllers):
    /// "the opponent" of either is the other. Only set inside a command.
    #[serde(skip)]
    pub(super) pair: Option<[u8; 2]>,
}

/// What an N-player duel starts from (the N-side form of
/// [`super::DuelSetup`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSetup {
    pub rules: MultiRules,
    pub terrain: Vec<u8>,
    pub leaders: Vec<u16>,
    pub decks: Vec<Vec<u16>>,
    pub first: u8,
    pub leader_abilities: Vec<Vec<u8>>,
    pub controllers: Vec<u8>,
}

impl Duel {
    /// The N-player state (a port addition); `None` in a 2-player duel.
    #[cfg(feature = "online")]
    pub fn multi(&self) -> Option<&Multi> {
        self.multi.as_deref()
    }
    /// The N-player state (a port addition); this build has none.
    #[cfg(not(feature = "online"))]
    pub fn multi(&self) -> Option<&Multi> {
        None
    }
    #[cfg(feature = "online")]
    fn multi_mut(&mut self) -> Option<&mut Multi> {
        self.multi.as_deref_mut()
    }
    #[cfg(not(feature = "online"))]
    fn multi_mut(&mut self) -> Option<&mut Multi> {
        None
    }

    /// A port addition: start an N-player duel (3 or 4 sides; 2v2 needs 4).
    #[cfg(feature = "online")]
    pub fn new_multi(setup: MultiSetup, cards: &impl CardData) -> Result<(Self, Vec<DuelEvent>)> {
        let n = setup.leaders.len();
        let teams_mode = setup.rules.mode == MultiMode::Teams;
        if !(3..=4).contains(&n)
            || (teams_mode && n != 4)
            || setup.decks.len() != n
            || setup.leader_abilities.len() != n
            || setup.controllers.len() != n
            || setup.terrain.len() != 49
            || setup.terrain.iter().any(|&t| t > 9)
            || usize::from(setup.first) >= n
            || setup.rules.round_limit == 0
        {
            return Err(DuelError::InvalidCommand("invalid N-player setup"));
        }
        let mut duel = Self::build(
            setup.terrain,
            &setup.leaders,
            &setup.decks,
            setup.first,
            &setup.leader_abilities,
            setup.controllers,
            &MULTI_LEADER_CELLS[..n],
            cards,
        )?;
        let lp = if setup.rules.start_lp == 0 {
            START_LP
        } else {
            setup.rules.start_lp.min(MAX_LP)
        };
        for side in &mut duel.sides {
            side.lp = lp;
        }
        let order: Vec<u8> = MULTI_TURN_ORDER
            .iter()
            .copied()
            .filter(|&s| usize::from(s) < n)
            .collect();
        let teams: Vec<u8> = (0..n as u8)
            .map(|s| if teams_mode { s / 2 } else { s })
            .collect();
        let mut multi = Multi {
            rules: setup.rules,
            teams,
            order,
            out: vec![None; n],
            targets: vec![0; n],
            pair: None,
        };
        multi.rules.start_lp = lp;
        duel.multi = Some(Box::new(multi));
        for s in 0..n as u8 {
            let t = duel.foes(s).first().copied().unwrap_or(s ^ 1);
            duel.multi.as_mut().expect("set above").targets[usize::from(s)] = t;
        }
        let mut events = Vec::new();
        duel.start_turn(setup.first, cards, &mut events)?;
        Ok((duel, events))
    }

    /// A client's copy of an N-player duel from what one seat may see
    /// (`roses_net::mirror_duel`): every living side needs its leader.
    #[cfg(feature = "online")]
    #[allow(clippy::too_many_arguments)]
    pub fn from_observed_multi(
        terrain: Vec<u8>,
        units: Vec<super::DuelUnit>,
        sides: Vec<super::SideState>,
        round: u8,
        first: u8,
        active: u8,
        multi: Multi,
        cards: &impl CardData,
    ) -> Result<Self> {
        let n = sides.len();
        if terrain.len() != 49
            || !(3..=4).contains(&n)
            || multi.out.len() != n
            || multi.teams.len() != n
            || multi.targets.len() != n
            || usize::from(first) >= n
            || usize::from(active) >= n
        {
            return Err(DuelError::InvalidCommand("invalid observed duel"));
        }
        for side in 0..n as u8 {
            let leaders = units.iter().filter(|u| u.side == side && u.slot == 0).count();
            if leaders > 1 || (leaders == 0 && multi.out[usize::from(side)].is_none()) {
                return Err(DuelError::InvalidCommand("each side needs exactly one leader"));
            }
        }
        if units
            .iter()
            .any(|u| u.position.index().is_none() || cards.card(u.card).is_none() || usize::from(u.side) >= n)
        {
            return Err(DuelError::InvalidCommand("observed unit is invalid"));
        }
        let next_unit = units.iter().map(|u| u.id.0).max().unwrap_or(0) + 1;
        Ok(Self {
            schema_version: super::DUEL_SCHEMA.into(),
            terrain,
            units,
            sides,
            round,
            first,
            active,
            pending_summon: None,
            outcome: None,
            experience: vec![Default::default(); n],
            controllers: vec![super::PAD_CONTROLLER; n],
            pending: None,
            temp_units: [None; 11],
            rng: 0,
            destiny_rows: None,
            leader_ranks: vec![0; n],
            records: Default::default(),
            chain_bound: Default::default(),
            deck_stranded: vec![Vec::new(); n],
            star_income: vec![super::STAR_INCOME; n],
            saved_decks: vec![Vec::new(); n],
            multi: Some(Box::new(multi)),
            next_unit,
        })
    }

    /// Number of sides (2, or 3..=4 in the port's N-player modes).
    pub fn side_count(&self) -> u8 {
        self.sides.len() as u8
    }

    /// The cell of `side`'s leader, if it is on the board (an N-player
    /// side knocked out with "cards leave" has none).
    pub fn leader_cell(&self, side: u8) -> Option<GridPos> {
        self.units
            .iter()
            .find(|u| u.side == side && u.slot == 0)
            .map(|u| u.position)
    }

    /// Whether `side` was knocked out of an N-player duel.
    pub fn is_out(&self, side: u8) -> bool {
        self.multi()
            .is_some_and(|m| m.out.get(usize::from(side)).is_some_and(Option::is_some))
    }

    /// A side still playing.
    pub fn alive(&self, side: u8) -> bool {
        side < self.side_count() && !self.is_out(side)
    }

    /// Two different sides of one 2v2 team.
    pub fn team_mates(&self, a: u8, b: u8) -> bool {
        a != b
            && self.multi().is_some_and(|m| {
                m.rules.mode == MultiMode::Teams
                    && m.teams.get(usize::from(a)) == m.teams.get(usize::from(b))
                    && m.teams.get(usize::from(a)).is_some()
            })
    }

    /// `b` is an enemy of `a` (2 players: any other side).
    pub fn foe(&self, a: u8, b: u8) -> bool {
        a != b && !self.team_mates(a, b)
    }

    /// The other sides in turn order, starting after `s` (2 players: the
    /// other side).
    fn after(&self, s: u8) -> Vec<u8> {
        let Some(m) = self.multi() else {
            return vec![s ^ 1];
        };
        let n = m.order.len();
        let i = m.order.iter().position(|&x| x == s).unwrap_or(0);
        (1..n).map(|k| m.order[(i + k) % n]).collect()
    }

    /// The living enemies of `s` whose cards answer for "the other side":
    /// inside a battle the battle partner, otherwise every living enemy in
    /// turn order after `s`. 2 players: `[s ^ 1]`.
    pub(super) fn foes(&self, s: u8) -> Vec<u8> {
        let Some(m) = self.multi() else {
            return vec![s ^ 1];
        };
        if let Some([a, b]) = m.pair {
            let partner = if s == a && b != a {
                Some(b)
            } else if s == b && a != b {
                Some(a)
            } else {
                None
            };
            if let Some(p) = partner {
                return if self.alive(p) { vec![p] } else { Vec::new() };
            }
        }
        self.after(s)
            .into_iter()
            .filter(|&o| self.foe(s, o) && self.alive(o))
            .collect()
    }

    /// "The opponent" of `s` for a single-target effect: the battle partner
    /// inside a battle, else `s`'s chosen target (the first living enemy
    /// after `s` when that one is gone). 2 players: `s ^ 1`.
    pub(super) fn opp(&self, s: u8) -> u8 {
        let Some(m) = self.multi() else {
            return s ^ 1;
        };
        if let Some([a, b]) = m.pair {
            if s == a && b != a {
                return b;
            }
            if s == b && a != b {
                return a;
            }
        }
        if let Some(&t) = m.targets.get(usize::from(s)) {
            if self.alive(t) && self.foe(s, t) {
                return t;
            }
        }
        let after = self.after(s);
        after
            .iter()
            .copied()
            .find(|&o| self.alive(o) && self.foe(s, o))
            .or_else(|| after.first().copied())
            .unwrap_or(s ^ 1)
    }

    /// The living enemies of `s` in turn order after `s`, battle or not
    /// (2 players: `[s ^ 1]`): the groups whose field-wide continuous
    /// effects reach `s`'s units.
    pub(super) fn enemy_groups(&self, s: u8) -> Vec<u8> {
        if self.multi().is_none() {
            return vec![s ^ 1];
        }
        self.after(s)
            .into_iter()
            .filter(|&o| self.foe(s, o) && self.alive(o))
            .collect()
    }

    /// [`Self::enemy_groups`] whose leader is on the board (the leader
    /// abilities #13..#16 read them).
    pub(super) fn enemy_leaders(&self, s: u8) -> Vec<u8> {
        if self.multi().is_none() {
            return vec![s ^ 1];
        }
        self.enemy_groups(s)
            .into_iter()
            .filter(|&o| self.leader_cell(o).is_some())
            .collect()
    }

    /// Every side whose cards can still react, in index order (2 players:
    /// `[0, 1]`).
    pub(super) fn living_sides(&self) -> Vec<u8> {
        (0..self.side_count()).filter(|&s| self.alive(s)).collect()
    }

    /// Every side (graveyards stay readable after a knockout).
    pub(super) fn all_sides(&self) -> Vec<u8> {
        (0..self.side_count()).collect()
    }

    /// `first`, then the other living sides in turn order (2 players:
    /// `[first, first ^ 1]`, the original's mask-3 search order).
    pub(super) fn first_then_all(&self, first: u8) -> Vec<u8> {
        if self.multi().is_none() {
            return vec![first, first ^ 1];
        }
        let mut out = vec![first];
        out.extend(self.after(first).into_iter().filter(|&o| self.alive(o)));
        out
    }

    /// The enemies of `s` ([`Self::foes`]), then `s`, then the other living
    /// sides (2 players: `[s ^ 1, s]`).
    pub(super) fn foes_then_all(&self, s: u8) -> Vec<u8> {
        if self.multi().is_none() {
            return vec![s ^ 1, s];
        }
        let mut out = self.foes(s);
        out.push(s);
        for o in self.after(s) {
            if self.alive(o) && !out.contains(&o) {
                out.push(o);
            }
        }
        out
    }

    /// The side whose turn follows `s`'s (2 players: `s ^ 1`).
    pub(super) fn next_side(&self, s: u8) -> u8 {
        if self.multi().is_none() {
            return s ^ 1;
        }
        self.after(s)
            .into_iter()
            .find(|&o| self.alive(o))
            .unwrap_or(s)
    }

    /// A turn passing from `prev` to `next` starts a new round (it reaches
    /// or passes the first player's seat).
    pub(super) fn wraps(&self, prev: u8, next: u8) -> bool {
        let Some(m) = self.multi() else {
            return next == self.first;
        };
        let n = m.order.len();
        let index = |s: u8| m.order.iter().position(|&x| x == s).unwrap_or(0);
        let first = index(self.first);
        let pos = |s: u8| (index(s) + n - first) % n;
        pos(next) <= pos(prev)
    }

    /// Set the battle partners for the duration of one battle.
    pub(super) fn set_pair(&mut self, pair: Option<[u8; 2]>) {
        if let Some(m) = self.multi_mut() {
            m.pair = pair;
        }
    }

    /// [`DuelCommand::Target`]: `side` aims its single-target effects at
    /// `target`.
    pub(super) fn choose_target(&mut self, side: u8, target: u8, events: &mut Vec<DuelEvent>) -> Result<()> {
        if self.multi().is_none() {
            return Err(DuelError::InvalidCommand("only an N-player duel has targets"));
        }
        if !self.alive(target) || !self.foe(side, target) {
            return Err(DuelError::Illegal("the target must be an enemy still playing"));
        }
        if let Some(m) = self.multi_mut() {
            m.targets[usize::from(side)] = target;
        }
        events.push(DuelEvent::TargetChosen { side, target });
        Ok(())
    }

    /// The N-player LP comparison at the round limit: the highest LP among
    /// the living players (2v2: the team total, or the shared LP); a tie is
    /// a draw.
    pub(super) fn multi_lp_winner(&self) -> Option<u8> {
        let m = self.multi()?;
        let alive = self.living_sides();
        let score = |s: u8| -> u32 {
            if m.rules.mode == MultiMode::Teams {
                let mates: Vec<u8> = alive
                    .iter()
                    .copied()
                    .filter(|&o| o == s || self.team_mates(s, o))
                    .collect();
                if m.rules.shared_lp {
                    mates.iter().map(|&o| u32::from(self.sides[usize::from(o)].lp)).max().unwrap_or(0)
                } else {
                    mates.iter().map(|&o| u32::from(self.sides[usize::from(o)].lp)).sum()
                }
            } else {
                u32::from(self.sides[usize::from(s)].lp)
            }
        };
        let best = alive.iter().map(|&s| score(s)).max()?;
        let leaders: Vec<u8> = alive.iter().copied().filter(|&s| score(s) == best).collect();
        let first = *leaders.first()?;
        leaders
            .iter()
            .all(|&s| s == first || self.team_mates(first, s))
            .then_some(first)
    }

    /// Knock `side` out: its turns are skipped from now on; with
    /// [`MultiRules::cards_leave`] its units leave the board (no events
    /// but [`DuelEvent::KnockedOut`], no effects, no experience; cards to
    /// its graveyard), and every unit it controls goes back to its owner.
    /// Ends the duel when one player or team is left.
    pub(super) fn knock_out(&mut self, side: u8, reason: EndReason, events: &mut Vec<DuelEvent>) {
        if self.multi().is_none() || self.is_out(side) || self.outcome.is_some() {
            return;
        }
        let place = self.living_sides().len() as u8;
        let round = self.round;
        let leave = {
            let m = self.multi_mut().expect("checked above");
            m.out[usize::from(side)] = Some(Knockout { round, reason, place });
            m.rules.cards_leave
        };
        events.push(DuelEvent::KnockedOut { side, reason });
        for u in &mut self.units {
            if u.stolen && u.side != side && u.controller() == side {
                u.stolen = false;
                u.stolen_by = None;
            }
        }
        if leave {
            let mut gone: Vec<super::DuelUnit> = Vec::new();
            self.units.retain(|u| {
                if u.side == side {
                    gone.push(u.clone());
                    false
                } else {
                    true
                }
            });
            gone.sort_by_key(|u| u.slot);
            for u in gone {
                if u.slot != 0 {
                    self.sides[usize::from(side)].graveyard.push(u.card);
                }
                self.records.remove(&u.id.0);
                self.chain_bound.remove(&u.id.0);
            }
        }
        if self.pending.as_ref().is_some_and(|_| self.active == side) {
            self.pending = None;
        }
        if self.active == side {
            self.pending_summon = None;
        }
        // One player (FFA) or one team (2v2) left: the duel ends.
        let alive = self.living_sides();
        let teams: Vec<u8> = {
            let m = self.multi().expect("checked above");
            let mut t: Vec<u8> = alive.iter().map(|&s| m.teams[usize::from(s)]).collect();
            t.dedup();
            t.sort_unstable();
            t.dedup();
            t
        };
        if teams.len() <= 1 {
            self.finish(alive.first().copied(), reason, events);
        }
    }

    /// 2v2 with shared LP: every LP change of a team member this command
    /// made applies to the team's one total (`before`: the LP before the
    /// command).
    fn share_team_lp(&mut self, before: &[u16]) {
        let Some(m) = self.multi() else {
            return;
        };
        if m.rules.mode != MultiMode::Teams || !m.rules.shared_lp {
            return;
        }
        let teams = m.teams.clone();
        let mut done: Vec<u8> = Vec::new();
        for (s, &team) in teams.iter().enumerate() {
            if done.contains(&team) {
                continue;
            }
            done.push(team);
            let members: Vec<usize> = (0..teams.len())
                .filter(|&o| teams[o] == team && self.alive(o as u8))
                .collect();
            if members.is_empty() {
                continue;
            }
            let base = i32::from(before[members[0]]);
            let delta: i32 = members
                .iter()
                .map(|&o| i32::from(self.sides[o].lp) - i32::from(before[o]))
                .sum();
            let lp = (base + delta).clamp(0, i32::from(MAX_LP)) as u16;
            for &o in &members {
                self.sides[o].lp = lp;
            }
            let _ = s;
        }
    }

    /// After every command of an N-player duel: shared team LP, knockouts at
    /// 0 LP (in turn order from the acting side), and the next turn when the
    /// acting side itself was knocked out.
    pub(super) fn multi_after(
        &mut self,
        before: &[u16],
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        if self.outcome.is_some() {
            return Ok(());
        }
        self.share_team_lp(before);
        let active = self.active;
        let mut check = vec![active];
        check.extend(self.after(active));
        let alive = self.living_sides();
        if !alive.is_empty() && alive.iter().all(|&s| self.sides[usize::from(s)].lp == 0) {
            // Everyone left fell at once: a draw.
            self.finish(None, EndReason::LifePoints, events);
            return Ok(());
        }
        for s in check {
            if self.alive(s) && self.sides[usize::from(s)].lp == 0 {
                self.knock_out(s, EndReason::LifePoints, events);
                if self.outcome.is_some() {
                    return Ok(());
                }
            }
        }
        if self.is_out(active) {
            self.pending = None;
            self.pending_summon = None;
            return self.advance_turn(active, cards, events);
        }
        Ok(())
    }

    /// The end of `s`'s turn in an N-player duel (after the turn-end
    /// effects and [`DuelEvent::TurnEnded`]): a trapped leader knocks `s`
    /// out; then the next living side's turn, or the round limit.
    pub(super) fn multi_end_turn(
        &mut self,
        s: u8,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        if self.leader_cell(s).is_some() && self.leader_trapped(s) {
            self.knock_out(s, EndReason::LeaderTrapped, events);
            if self.outcome.is_some() {
                return Ok(());
            }
        }
        self.advance_turn(s, cards, events)
    }

    /// Hand the turn on from `s` (or end the duel at the round limit).
    pub(super) fn advance_turn(&mut self, s: u8, cards: &impl CardData, events: &mut Vec<DuelEvent>) -> Result<()> {
        let limit = self.multi().map_or(u8::MAX, |m| m.rules.round_limit);
        let next = self.next_side(s);
        if self.wraps(s, next) && self.round >= limit {
            let winner = self.multi_lp_winner();
            self.finish(winner, EndReason::TurnLimit, events);
            return Ok(());
        }
        self.start_turn(next, cards, events)
    }

    /// [`DuelCommand::Surrender`] / [`Duel::forfeit`] in an N-player duel:
    /// `side` is knocked out and play goes on.
    pub(super) fn multi_surrender(
        &mut self,
        side: u8,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let was_active = self.active == side;
        self.knock_out(side, EndReason::Surrender, events);
        if self.outcome.is_none() && was_active {
            self.pending = None;
            self.pending_summon = None;
            self.advance_turn(side, cards, events)?;
        }
        Ok(())
    }
}

#[cfg(feature = "online")]
pub mod computer;
#[cfg(all(test, feature = "online"))]
mod tests;
