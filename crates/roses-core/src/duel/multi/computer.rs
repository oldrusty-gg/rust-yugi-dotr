//! Port addition: computer players for the N-player duels
//! (MULTIPLAYER-DESIGN.md §5). The faithful computer modules
//! ([`super::super::ai::weevil`]) are 2-player ports and are not changed:
//! before each decision the N-side duel is turned into a 2-side duel
//! against one chosen enemy (the *focus*). The computer's side and the
//! focus keep their units (renumbered as groups 0 and 1); every other unit
//! (team-mates, the other enemies, a knocked-out player's obstacles)
//! becomes a Labyrinth wall in the projected terrain, which nothing can
//! enter, matching the rule that those cells are blocked. The module plans
//! on that projection and its action is mapped back (unit ids and cells
//! are the same in both). A refused action is skipped for the rest of the
//! turn; a turn that cannot be planned ends.
//!
//! The focus [INVENTED]: an enemy that can hit the computer's (or, in
//! 2v2, its team-mate's) leader for at least that player's LP next turn;
//! else the enemy with the lowest LP whose leader one of the computer's
//! monsters can reach; else the nearest enemy leader. It is chosen again
//! at every turn start, and the computer's single-target effects aim at it
//! ([`DuelCommand::Target`]).

use super::super::ai::weevil::{self, FusionData, Plan};
use super::super::ai::{self, Action, Position};
use super::super::{terrain, CardData, Catalogue, Duel, DuelCommand, Pending, Pick};
use crate::{GridPos, UnitId};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The control byte the computer players plan with by default: Custom
/// Duel's computer (the generic module, [`weevil::VS_CONTROLLERS`]).
pub const DEFAULT_CONTROLLER: u8 = 22;

/// Plans one computer turn may make before it ends the turn.
const PLANS_PER_TURN: u32 = 48;

/// One computer-played side of an N-player duel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiComputer {
    pub side: u8,
    /// The module's control byte ([`weevil::plan_for`]).
    pub controller: u8,
    pub tier: i16,
    /// The enemy this turn is planned against.
    pub focus: u8,
    memory: weevil::Memory,
    /// The projection's side and focus the memory belongs to.
    memory_for: Option<(u8, u8)>,
    /// The module's rand() state (kept here: the computer never writes the
    /// duel state outside a command).
    rng: u64,
    queue: VecDeque<DuelCommand>,
    /// Units refused or set aside this turn.
    skip: Vec<UnitId>,
    /// The turn being played (round, side).
    turn: Option<(u8, u8)>,
    plans: u32,
    refusals: u32,
    no_summon: bool,
    /// The Down count / graveyard of the last activation (its pick).
    pick_hint: Option<(Option<u8>, u16)>,
}

impl MultiComputer {
    pub fn new(duel: &Duel, side: u8) -> Self {
        Self::with_controller(duel, side, DEFAULT_CONTROLLER, 0)
    }

    pub fn with_controller(duel: &Duel, side: u8, controller: u8, tier: i16) -> Self {
        Self {
            side,
            controller,
            tier,
            focus: duel.opp(side),
            memory: weevil::Memory::default(),
            memory_for: None,
            rng: duel.rng ^ (0x9E37_79B9_7F4A_7C15_u64.wrapping_mul(u64::from(side) + 1)),
            queue: VecDeque::new(),
            skip: Vec::new(),
            turn: None,
            plans: 0,
            refusals: 0,
            no_summon: false,
            pick_hint: None,
        }
    }

    /// The next command for this side, or `None` when it is not this
    /// side's turn (or the duel is over).
    pub fn decide(
        &mut self,
        duel: &Duel,
        cards: &impl CardData,
        fusions: &impl FusionData,
    ) -> Option<DuelCommand> {
        if duel.outcome.is_some() || duel.active != self.side || !duel.alive(self.side) {
            return None;
        }
        let turn = (duel.round, duel.active);
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.queue.clear();
            self.skip.clear();
            self.plans = 0;
            self.refusals = 0;
            self.no_summon = false;
            self.pick_hint = None;
            self.focus = choose_focus(duel, self.side, cards);
        }
        if let Some(pending) = &duel.pending {
            return Some(self.answer(duel, pending, cards));
        }
        if duel.pending_summon.is_some() && self.queue.is_empty() {
            return Some(DuelCommand::CancelSummon);
        }
        if let Some(c) = self.queue.pop_front() {
            return Some(c);
        }
        if !duel.alive(self.focus) || !duel.foe(self.side, self.focus) {
            self.focus = choose_focus(duel, self.side, cards);
        }
        if duel.multi().is_some_and(|m| m.targets[usize::from(self.side)] != self.focus)
            && duel.alive(self.focus)
            && duel.foe(self.side, self.focus)
        {
            return Some(DuelCommand::Target { side: self.focus });
        }
        for _ in 0..4 {
            self.plans += 1;
            if self.plans > PLANS_PER_TURN || self.refusals > 8 {
                return Some(DuelCommand::EndTurn);
            }
            let (projected, ps) = project(duel, self.side, self.focus, &self.skip);
            if self.memory_for != Some((ps, self.focus)) {
                self.memory = weevil::Memory::new(&projected, ps);
                self.memory_for = Some((ps, self.focus));
            }
            let mut p = projected;
            p.rng = self.rng;
            weevil::sync(&mut self.memory, &p, ps);
            let plan = weevil::plan_for_rng(self.controller, &p, ps, &mut self.memory, cards, fusions, self.tier);
            let Ok((plan, rng)) = plan else {
                return Some(DuelCommand::EndTurn);
            };
            self.rng = rng;
            match plan {
                Plan::EndTurn => return Some(DuelCommand::EndTurn),
                Plan::NoKey => continue,
                Plan::MarkActed(id) => {
                    self.skip.push(id);
                    continue;
                }
                Plan::Act(action) => {
                    if let Some(c) = self.command_of(&p, &action, cards) {
                        return Some(c);
                    }
                    if let Some(u) = actor(&action) {
                        self.skip.push(u);
                    } else {
                        self.no_summon = true;
                    }
                }
            }
        }
        Some(DuelCommand::EndTurn)
    }

    /// The duel refused the last command: set its unit aside for the turn
    /// (or stop summoning).
    pub fn refused(&mut self, command: &DuelCommand) {
        self.refusals += 1;
        self.queue.clear();
        match command {
            DuelCommand::Move { unit, .. } => self.skip.push(*unit),
            DuelCommand::OpenSummon { .. } | DuelCommand::ChooseSummon { .. } => self.no_summon = true,
            _ => {}
        }
    }

    fn command_of(&mut self, p: &Duel, action: &Action, cards: &impl CardData) -> Option<DuelCommand> {
        match action {
            Action::Summon { cell, hand } => {
                if self.no_summon {
                    return None;
                }
                self.queue.push_back(DuelCommand::ChooseSummon { hand: hand.clone() });
                Some(DuelCommand::OpenSummon { cell: *cell })
            }
            Action::Act {
                unit,
                target,
                reveal,
                position,
            } => {
                let u = p.unit(*unit)?;
                let holds = *target == u.position;
                let defence = if holds && u.slot != 0 {
                    match position {
                        Position::Attack if u.committed_defence => Some(false),
                        Position::Defence if !u.committed_defence => Some(true),
                        _ => None,
                    }
                } else {
                    None
                };
                Some(DuelCommand::Move {
                    unit: *unit,
                    path: if holds { Vec::new() } else { ai::path(p, *unit, *target, cards) },
                    face_up: *reveal && u.slot != 0 && !u.face_up,
                    defence,
                })
            }
            Action::Activate {
                unit, graveyard, down, ..
            } => {
                self.pick_hint = Some((*graveyard, *down));
                Some(DuelCommand::Move {
                    unit: *unit,
                    path: Vec::new(),
                    face_up: true,
                    defence: None,
                })
            }
        }
    }

    /// Answer an open selection: the planned pick when it fits, else the
    /// first enemy unit / the strongest offered monster, else nothing.
    fn answer(&mut self, duel: &Duel, pending: &Pending, cards: &impl CardData) -> DuelCommand {
        let hint = self.pick_hint.take();
        let pick = match pending {
            Pending::ControlChange { by, units } => {
                let planned = hint.and_then(|(_, down)| units.get(usize::from(down))).copied();
                planned
                    .filter(|id| duel.unit(*id).is_some_and(|u| duel.foe(u.controller(), *by)))
                    .or_else(|| {
                        units
                            .iter()
                            .copied()
                            .find(|id| duel.unit(*id).is_some_and(|u| duel.foe(u.controller(), *by)))
                    })
                    .map_or(Pick::Cancel, |unit| Pick::Unit { unit })
            }
            Pending::GraveyardCopy {
                graveyards,
                monsters,
                non_monsters,
                ..
            } => {
                let mut best: Option<(u16, u8, usize)> = None;
                for &g in graveyards {
                    for (i, &c) in duel.sides[usize::from(g)].graveyard.iter().enumerate() {
                        let Some(info) = cards.card(c) else { continue };
                        if (*monsters && !info.is_monster()) || (*non_monsters && c < 0x2AB) {
                            continue;
                        }
                        let value = if info.is_monster() { info.attack } else { 1 };
                        if best.is_none_or(|(v, _, _)| value > v) {
                            best = Some((value, g, i));
                        }
                    }
                }
                best.map_or(Pick::Cancel, |(_, side, index)| Pick::Graveyard { side, index })
            }
        };
        DuelCommand::Pick { pick }
    }
}

fn actor(action: &Action) -> Option<UnitId> {
    match action {
        Action::Summon { .. } => None,
        Action::Act { unit, .. } | Action::Activate { unit, .. } => Some(*unit),
    }
}

fn manhattan(a: GridPos, b: GridPos) -> u8 {
    a.column.abs_diff(b.column) + a.row.abs_diff(b.row)
}

/// The focus rule [INVENTED] (module docs).
pub fn choose_focus(duel: &Duel, side: u8, cards: &impl CardData) -> u8 {
    let enemies: Vec<u8> = (0..duel.side_count())
        .filter(|&o| duel.alive(o) && duel.foe(side, o) && duel.leader_cell(o).is_some())
        .collect();
    if enemies.is_empty() {
        return duel.opp(side);
    }
    let lp = |s: u8| duel.sides[usize::from(s)].lp;
    let attack = |u: &super::super::DuelUnit| {
        cards
            .card(u.card)
            .filter(|c| c.is_monster() && u.slot != 0)
            .map_or(0, |c| (i32::from(c.attack) + i32::from(u.attack_bonus)).max(0) as u32)
    };
    // 1. A threat to the own (or the team-mate's) leader.
    let guarded: Vec<u8> = (0..duel.side_count())
        .filter(|&o| (o == side || duel.team_mates(side, o)) && duel.alive(o))
        .collect();
    let mut threats: Vec<(u32, u8)> = Vec::new();
    for &g in &guarded {
        let Some(at) = duel.leader_cell(g) else { continue };
        for u in &duel.units {
            let ctrl = u.controller();
            if !enemies.contains(&ctrl) || !u.committed_face_up {
                continue;
            }
            let a = attack(u);
            if a >= u32::from(lp(g)) && manhattan(u.position, at) <= 3 {
                threats.push((a, ctrl));
            }
        }
    }
    if let Some(&(_, s)) = threats.iter().max_by_key(|(a, s)| (*a, std::cmp::Reverse(*s))) {
        return s;
    }
    // 2. The weakest enemy whose leader the own monsters can reach.
    let reachable: Vec<u8> = enemies
        .iter()
        .copied()
        .filter(|&e| {
            let at = duel.leader_cell(e).expect("filtered");
            duel.units
                .iter()
                .any(|u| u.controller() == side && u.slot != 0 && attack(u) > 0 && manhattan(u.position, at) <= 2)
        })
        .collect();
    if let Some(&e) = reachable.iter().min_by_key(|&&e| (lp(e), e)) {
        return e;
    }
    // 3. The nearest enemy leader (then the lowest LP).
    let own = duel.leader_cell(side).unwrap_or(GridPos { column: 3, row: 3 });
    *enemies
        .iter()
        .min_by_key(|&&e| (manhattan(own, duel.leader_cell(e).expect("filtered")), lp(e), e))
        .expect("not empty")
}

/// The 2-side duel `side` plans its turn on against `focus` (module docs):
/// returns it and the group `side` plays as in it (0 when its leader is
/// south of or level with the focus's and east of it, else 1). Units in
/// `skip` are marked acted.
pub fn project(duel: &Duel, side: u8, focus: u8, skip: &[UnitId]) -> (Duel, u8) {
    let own = duel.leader_cell(side).unwrap_or(GridPos { column: 3, row: 6 });
    let enemy = duel.leader_cell(focus).unwrap_or(GridPos { column: 3, row: 0 });
    let ps: u8 = if own.row > enemy.row || (own.row == enemy.row && own.column >= enemy.column) {
        0
    } else {
        1
    };
    let pf = ps ^ 1;
    let map = |s: u8| {
        if s == side {
            Some(ps)
        } else if s == focus {
            Some(pf)
        } else {
            None
        }
    };
    let pick = |a: usize, b: usize| if ps == 0 { [a, b] } else { [b, a] };
    let [i0, i1] = pick(usize::from(side), usize::from(focus));
    let mut p = duel.clone();
    p.multi = None;
    p.sides = vec![duel.sides[i0].clone(), duel.sides[i1].clone()];
    p.experience = vec![duel.experience[i0].clone(), duel.experience[i1].clone()];
    p.controllers = vec![duel.controllers[i0], duel.controllers[i1]];
    p.leader_ranks = vec![duel.leader_ranks[i0], duel.leader_ranks[i1]];
    p.star_income = vec![duel.star_income[i0], duel.star_income[i1]];
    p.saved_decks = vec![duel.saved_decks[i0].clone(), duel.saved_decks[i1].clone()];
    p.deck_stranded = vec![duel.deck_stranded[i0].clone(), duel.deck_stranded[i1].clone()];
    p.units.clear();
    for u in &duel.units {
        match (map(u.side), map(u.controller())) {
            (Some(owner), Some(ctrl)) => {
                let mut v = u.clone();
                v.side = owner;
                v.stolen = owner != ctrl;
                v.stolen_by = None;
                if skip.contains(&u.id) {
                    v.acted = true;
                }
                p.units.push(v);
            }
            _ => {
                if let Some(i) = u.position.index() {
                    p.terrain[i] = terrain::LABYRINTH;
                }
            }
        }
    }
    p.active = map(duel.active).unwrap_or(ps);
    p.first = map(duel.first).unwrap_or(ps);
    p.pending = None;
    p.pending_summon = None;
    (p, ps)
}

/// [`FusionData`] from the imported catalogue alone (fusions, power-up
/// fits and plain bonuses), for computer players where the executable's
/// tables ([`weevil::Tables`]) are not at hand (tests, synthetic data).
pub struct CatalogueFusions<'a>(pub &'a Catalogue);

impl FusionData for CatalogueFusions<'_> {
    fn fuse(&self, a: u16, b: u16) -> Option<u16> {
        self.0.fusion(a, b).flatten()
    }
    fn equip(&self, monster: u16, card: u16) -> i32 {
        let (Some(m), Some(_)) = (self.0.card(monster), self.0.card(card)) else {
            return -1;
        };
        if super::super::power_up_fits(m, card) {
            i32::from(monster) | 0x10000
        } else {
            -1
        }
    }
    fn category(&self, card: u16) -> u8 {
        self.0.card(card).and_then(|c| c.power_up).map_or(0, |p| p.category)
    }
    fn value(&self, card: u16) -> u16 {
        self.0.card(card).and_then(|c| c.power_up).map_or(0, |p| p.value)
    }
}

/// Play an N-player duel to its end with a computer on every side listed
/// in `computers` (tests and server-side checks). Returns the duel when it
/// ended or `max_commands` commands were applied.
pub fn play_out(
    mut duel: Duel,
    computers: &mut [MultiComputer],
    cards: &impl CardData,
    fusions: &impl FusionData,
    max_commands: usize,
) -> Duel {
    let mut stalls = 0;
    for _ in 0..max_commands {
        if duel.outcome.is_some() {
            break;
        }
        let side = duel.active;
        let Some(c) = computers.iter_mut().find(|c| c.side == side) else {
            break;
        };
        let Some(command) = c.decide(&duel, cards, fusions) else {
            break;
        };
        match duel.apply(side, &command, cards) {
            Ok((next, _)) => {
                duel = next;
                stalls = 0;
            }
            Err(_) => {
                c.refused(&command);
                stalls += 1;
                if stalls > 64 {
                    // Nothing this side tries is accepted (an interaction the
                    // port refuses): it leaves the duel.
                    match duel.forfeit_with(side, cards) {
                        Ok((next, _)) => duel = next,
                        Err(_) => break,
                    }
                    stalls = 0;
                }
            }
        }
    }
    duel
}
