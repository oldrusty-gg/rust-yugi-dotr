//! Per-card experience (private research `claude-end/DUEL-END.md` §2).
//!
//! The only duel "score" of the original is an experience value per side and
//! card id: u16 at card-block record +0x4A, changed by 0x2152A0(side, card,
//! op, v) and clamped to 0..=0xFFFF after every call. Side 2 (the summon
//! chain's temporary group), card 0x29F (mapped to 999) and ids >= 854 are
//! ignored by that function. The engine never sees the save data, so it
//! records what each call would do ([`ExperienceChange`]) and leaves applying
//! it to the campaign layer. Ranks used during the duel stay the stored ones;
//! they are recomputed only after the duel ([`recompute_rank`]).
//!
//! Every call site of 0x2152A0 in the executable, and where the engine makes
//! the same change. Checked in the EE interpreter from saved states: battle
//! results, leader hits, attacks on set cards, Bear Trap, crush, moves onto
//! own units, the summon chain, the end of a turn and the duel end; read
//! from the code only: Negate/Block Attack, Man-Eater Bug, magic, moth
//! transformation, rituals and a leader moving onto an own unit.
//! - battle comparison 0x248130: [`Duel::battle_experience`], the
//!   leader hit handler 0x275790 ([`Duel::leader_hit_experience`]) and
//!   attacks on set cards (+6 at 0x2495D4 / 0x249810, in `battle`);
//! - removal by an effect 0x250C90 (unit's side) and the summon chain's
//!   0x250D50 (active side): -4 ([`Duel::remove_by_effect`], `discard`,
//!   `choose_summon`);
//! - end of the own turn 0x273E20 phase 3 ([`Duel::end_turn_experience`]);
//! - duel end 0x2745F0 ([`Duel::end_award`]).
//!
//! Battle deaths get nothing: steps 11/15 of 0x24AFA0 call 0x2152A0(g,
//! 0x1E22B0(g, slot), 1, -4) only after 0x1E0E30 has reset the unit (slot
//! 0xB), so the card read is -1 and the call does nothing (0x24C5A8,
//! 0x24C64C, 0x24CA58, 0x24CAFC; interpreter: a destroyed unit's card keeps
//! its experience).

use super::{
    CardData, Duel, DuelEvent, DuelUnit, EndReason, Outcome, Result, MONSTER_SLOTS,
    REJECTED_DECK_CARD,
};
use crate::{UnitId, CARD_COUNT};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Entries of the rank threshold table (u16[13] at EE 0x2D0FC0).
pub const RANK_COUNT: usize = 13;
/// 0x2154C0/0x215570 give no rank to ids >= 0x2AB (non-monsters).
pub const RANKED_CARDS: u16 = 0x2AB;

/// The combined effect of a series of 0x2152A0 add calls on one stored
/// value. Each call clamps to 0..=0xFFFF, and clamped additions compose to
/// `clamp(stored + delta, low, high)`, so this is exact for any stored value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperienceChange {
    /// Sum of the added values.
    pub delta: i32,
    /// Lower and upper bounds of the composed clamp.
    pub low: u16,
    pub high: u16,
}
impl Default for ExperienceChange {
    fn default() -> Self {
        Self {
            delta: 0,
            low: 0,
            high: u16::MAX,
        }
    }
}
impl ExperienceChange {
    /// One more 0x2152A0(side, card, 0, value) call.
    pub fn add(&mut self, value: i32) {
        let clamp = |v: u16| (i32::from(v) + value).clamp(0, i32::from(u16::MAX)) as u16;
        self.delta = self.delta.saturating_add(value);
        self.low = clamp(self.low);
        self.high = clamp(self.high);
    }
    /// The stored experience after all recorded calls.
    pub fn apply(&self, stored: u16) -> u16 {
        (i64::from(stored) + i64::from(self.delta)).clamp(i64::from(self.low), i64::from(self.high))
            as u16
    }
}

/// One 0x2152A0 call made by the duel-end award 0x2745F0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperienceAward {
    pub side: u8,
    pub card: u16,
    pub value: i32,
}

/// Whether 0x2152A0 accepts the card: 0x29F becomes 999 and ids >= 854 are
/// ignored.
fn counted(card: u16) -> bool {
    card != REJECTED_DECK_CARD && card < CARD_COUNT
}

/// D51, the end reason code 0x2745F0 switches on (turn-verified §7).
fn reason_code(reason: EndReason) -> usize {
    match reason {
        EndReason::LifePoints | EndReason::Surrender => 0,
        EndReason::TurnLimit => 1,
        EndReason::Exodia => 2,
        EndReason::LeaderTrapped => 3,
    }
}

/// 0x2745F0 leader values by reason code and graveyard bracket
/// (count < 5, < 10, < 15, else): the winner's leader, the loser's leader.
const WINNER_LEADER: [[i32; 4]; 4] = [
    [40, 32, 24, 16],
    [32, 24, 16, 8],
    [44, 36, 28, 20],
    [48, 40, 32, 24],
];
const LOSER_LEADER: [[i32; 4]; 4] = [
    [-32, -24, -16, -8],
    [-28, -20, -12, -4],
    [-36, -28, -20, -12],
    [-40, -32, -24, -16],
];

fn bracket(graveyard: usize) -> usize {
    (graveyard / 5).min(3)
}

/// Rank recompute 0x215570 (result state 0x5E): the largest r with
/// `experience >= thresholds[r]`; a card whose stored rank (the 14-bit field
/// at record +0x48) is not 0 never drops below 1. `None` for cards without a
/// rank (ids >= 0x2AB, and 0x29F, which is read as 999). The thresholds are
/// the u16[13] at EE 0x2D0FC0 (`roses_assets::parse_pal_rank_thresholds`).
pub fn recompute_rank(
    card: u16,
    experience: u16,
    stored_rank: u16,
    thresholds: &[u16; RANK_COUNT],
) -> Option<u16> {
    if card == REJECTED_DECK_CARD || card >= RANKED_CARDS {
        return None;
    }
    let mut rank = RANK_COUNT as i32 - 1;
    while rank >= 0 && experience < thresholds[rank as usize] {
        rank -= 1;
    }
    if stored_rank & 0x3FFF != 0 && rank < 2 {
        rank = 1;
    }
    Some((rank & 0x3FFF) as u16)
}

impl Duel {
    /// 0x2152A0(side, card, 0, value).
    pub(super) fn record_experience(&mut self, side: u8, card: u16, value: i32) {
        if side < 2 && counted(card) {
            self.experience[usize::from(side)]
                .entry(card)
                .or_default()
                .add(value);
        }
    }

    /// cost(u) = 0x1E1B70(g, slot, 0) = clamp(level - U+0x48, 0, 12); U+0x48
    /// is the ability-8 summon discount stored when the record was made.
    fn experience_cost(&self, unit: &DuelUnit, cards: &impl CardData) -> i32 {
        i32::from(self.unit_cost(unit, cards))
    }

    /// 0x248130 for two monsters, the defender not a leader, `a`/`d` the
    /// compared values and `dealt` the LP loss actually inflicted. A
    /// survivor that dealt damage gets 4·cost(loser), otherwise
    /// 2·(cost(loser) + 2): the difference is not dealt when rec+0x10 bit
    /// 0/1 (the type-5 shield of the loser, F03.md §2.5) suppresses it.
    /// Outcomes decided by the pre-pass (0x2485D4, 0x24860C, 0x2486D8,
    /// 0x2487A0) cannot occur yet.
    pub(super) fn battle_experience(
        &mut self,
        attacker: &DuelUnit,
        defender: &DuelUnit,
        a: u16,
        d: u16,
        dealt: u16,
        cards: &impl CardData,
    ) {
        let (att_cost, def_cost) = (
            self.experience_cost(attacker, cards),
            self.experience_cost(defender, cards),
        );
        let won = |loser: i32| {
            if dealt > 0 {
                4 * loser
            } else {
                2 * (loser + 2)
            }
        };
        if defender.defence {
            // DEF broken (0x248CD4); DEF held (D >= A) gives nothing.
            if d < a {
                self.record_experience(attacker.side, attacker.card, 2 * (def_cost + 2));
            }
        } else if d > a {
            // 0x2490AC (damage dealt) / 0x249070.
            self.record_experience(defender.side, defender.card, won(att_cost));
        } else if d == a {
            // 0x2491A4, then 0x2491DC.
            self.record_experience(defender.side, defender.card, 2 * (att_cost + 2));
            self.record_experience(attacker.side, attacker.card, 2 * (def_cost + 2));
        } else {
            // 0x24933C (damage dealt) / 0x249300.
            self.record_experience(attacker.side, attacker.card, won(def_cost));
        }
    }

    /// Leader hit, event 0x5D handler 0x275790 after the LP loss: the
    /// attacker's card +20 (0x275E4C), or +24 when that side's LP is now 0
    /// (0x275E80); the hit leader's card -8 (0x275EA0).
    pub(super) fn leader_hit_experience(&mut self, attacker: &DuelUnit, leader: &DuelUnit) {
        let value = if self.sides[usize::from(leader.side)].lp > 0 {
            20
        } else {
            24
        };
        self.record_experience(attacker.side, attacker.card, value);
        self.record_experience(leader.side, leader.card, -8);
    }

    /// 0x250C90: a unit removed by an effect (trap, flip effect, crush event
    /// 0x70, transformation, ritual sacrifice, the target of a move onto an
    /// own unit) charges its card 4 on its own side before it goes.
    pub(super) fn remove_by_effect(
        &mut self,
        id: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        if let Some(u) = self.unit(id) {
            let (side, card) = (u.side, u.card);
            self.record_experience(side, card, -4);
        }
        self.destroy(id, cards, events)
    }

    /// End of `side`'s turn, 0x273E20 phase 3 (0x274118), after the crush
    /// sweep and before the trapped-leader and turn-limit checks: each unit
    /// in slots 1..=5 gets +4 if committed face-up, else +2.
    pub(super) fn end_turn_experience(&mut self, side: u8) {
        for slot in 1..=MONSTER_SLOTS as u8 {
            let Some(u) = self.units.iter().find(|u| u.side == side && u.slot == slot) else {
                continue;
            };
            let (card, value) = (u.card, if u.committed_face_up { 4 } else { 2 });
            self.record_experience(side, card, value);
        }
    }

    /// The duel-end award 0x2745F0(D50, D51), run at result state 0x5D, in
    /// call order: the winner's slots 1..=5 (+6 committed face-up, else +4),
    /// the winner's leader, then the loser's leader, by end reason and by
    /// that side's graveyard count. On a draw (D50 = 2) nobody wins and side
    /// 0 is charged as the loser (0x274698 picks side 0 for any D50 != 0).
    pub fn end_award(&self, outcome: Outcome) -> Vec<ExperienceAward> {
        let reason = reason_code(outcome.reason);
        let mut out = Vec::new();
        let mut push = |side: u8, card: u16, value: i32| {
            if counted(card) {
                out.push(ExperienceAward { side, card, value });
            }
        };
        let leader_of = |side: u8| {
            self.units
                .iter()
                .find(|u| u.side == side && u.slot == 0)
                .map(|u| u.card)
        };
        let graveyard = |side: u8| bracket(self.sides[usize::from(side)].graveyard.len());
        if let Some(winner) = outcome.winner {
            for slot in 1..=MONSTER_SLOTS as u8 {
                if let Some(u) = self
                    .units
                    .iter()
                    .find(|u| u.side == winner && u.slot == slot)
                {
                    push(winner, u.card, if u.committed_face_up { 6 } else { 4 });
                }
            }
            if let Some(card) = leader_of(winner) {
                push(winner, card, WINNER_LEADER[reason][graveyard(winner)]);
            }
        }
        let loser = if outcome.winner == Some(0) { 1 } else { 0 };
        if let Some(card) = leader_of(loser) {
            push(loser, card, LOSER_LEADER[reason][graveyard(loser)]);
        }
        out
    }

    /// Everything this duel does to the two card blocks: the changes
    /// recorded during play followed by the duel-end award (when the duel has
    /// ended). Apply with [`ExperienceChange::apply`], then recompute ranks.
    pub fn final_experience(&self) -> [BTreeMap<u16, ExperienceChange>; 2] {
        let mut out: [BTreeMap<u16, ExperienceChange>; 2] = [self.experience[0].clone(), self.experience[1].clone()];
        if let Some(outcome) = self.outcome {
            for award in self.end_award(outcome) {
                out[usize::from(award.side)]
                    .entry(award.card)
                    .or_default()
                    .add(award.value);
            }
        }
        out
    }
}
