//! Duel-rule getters the HUD reads every frame, as pure functions of the
//! engine state, traced from the PAL executable (private research:
//! `work/research/duel/claude-screen/hud/`, `NOTES.md` "Rust port").
//!
//! | original | here |
//! |---|---|
//! | 0x1DA6A0(unit) | [`Query::hidden`] (viewer = d3c) |
//! | 0x1E1C60(unit, 0) != 0 | [`Query::has_effect`] |
//! | 0x1E1EF0(unit, a, 0) | [`Query::ability`], [`Query::abilities`] |
//! | 0x268CA0(unit, viewer) | [`Query::legend_query`] |
//! | 0x267A00 + 0x1E1570/0x1E1650, 0x24F7D0, 0x247E10 at a cell | [`Query::stats`] |
//! | 0x2692E0(A.side, A.slot, B.side, B.slot) | [`Query::advantage`] |
//! | 0x251C80(side, slot) | [`Query::controller`] |
//! | 0x1E1B70(2, i, 0) on a hand record | [`Query::hand_card_cost`] |
//! | 0x270280() | [`Query::marked_cost`] |
//!
//! The engine keeps no unit records for hand cards, no option words and no
//! pad state: [`Context`] carries what
//! the getters read of those and [`Record`] names the record a getter takes.
//! Other assumptions, each checked against the recorded savestates by the
//! private `hud_queries_check`:
//! - A record's effect entries (+0x08..+0x2F) are its card's
//!   [`CardInfo::entries`] (unit creation 0x1DDFE0; a `None` entry is the
//!   all-ones sentinel, and every card with an effect index has at least one
//!   entry). A leader's area holds its ability row instead (table 0x2C26B8 +
//!   card·40), which the engine keeps evaluated as
//!   [`super::SideState::leader_abilities`]; see [`leader_abilities`].
//! - Leader records normally have +0x48 (summon discount) 0; observed
//!   records retain their stored [`DuelUnit::discount`] in level queries.
//! - Leaders never hold committed defence (+0x57 bit 5), so they are never
//!   the type-6 movement sources 0x247E10 scans in slot 0.
//! - Filter 0x31 of 0x2505F0 is 0x24A0C0(card, 0x2FF) != 0x3E7, i.e.
//!   [`power_up_fits`] for power-up 0x2FF (category 0, value 500).
//! - The leader flags are read as the engine keeps them: [`Duel::new`] marks
//!   leaders face-up and committed, which the original's leader records
//!   usually do not (+0x57 bits 2/4 are clear in the Weevil duel, set in
//!   savestate 600); only 0x1DA6A0 on a leader reads them, and the HUD does
//!   not show that result.

use super::{
    attribute_relation, power_up_fits, terrain_affinity, CardData, CardInfo, Duel, DuelUnit,
};
use crate::{GridPos, UnitId};
use serde::{Deserialize, Serialize};

/// Card 0x29F ("Summoned Lord Exodia"): no card record for the getters.
const NO_RECORD_CARD: u16 = 0x29F;
/// 0x356: first id past the card table.
const CARD_LIMIT: u16 = 0x356;
/// Card 0x2A3 takes a leader's value instead of its own (0x267A00).
const LEADER_COPY_CARD: u16 = 0x2A3;

/// Original state the getters read that the engine does not keep. Every
/// recorded duel has `option_28 == option_29 == 3`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    /// 0x1A8C50(0x1C) (word 0x2DAB00): bit `side` enables 0x1E1EF0 for
    /// records of that side.
    pub option_28: u32,
    /// 0x1A8C50(0x1D) (word 0x2DAB04): bit `side` selects the controller
    /// type rule of 0x1DA6A0 for records of that side.
    pub option_29: u32,
    /// Side +0x33C (0x1DF760): the controller type (< 32 a computer script,
    /// >= 100 a pad; Weevil is 2).
    pub controllers: [u8; 2],
    /// Units whose +0x57 bit 1 is set: controlled by the other side.
    pub control_changed: Vec<UnitId>,
    /// 0x203380(port, 1, 2): R2 (pad bit 0x0002) held on pad port `side`.
    pub r2_held: [bool; 2],
}

/// The unit record a getter takes (EE 0x2DB990 + group·0x3F4 + slot·0x5C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record<'a> {
    /// A field unit (groups 0 and 1) of [`Duel::units`].
    Field(&'a DuelUnit),
    /// A hand card's record (group 2, built by 0x1D9C70 for the hand menu):
    /// `card` of `side`'s hand in slot `slot` (hand index + 1). Such records
    /// are off the board, face-down, without flags or changes; their
    /// +0x48 is 0x2703B0(side, card).
    Hand { side: u8, slot: u8, card: u16 },
}

/// The values a card box shows for a record at one cell (HUD `UnitStats`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    /// 0x267A00(unit, col, row, 1) + 0x1E1570(unit, 0), unclamped (the HUD
    /// clamps to 0..=9999).
    pub attack: i32,
    /// 0x267A00(unit, col, row, 0) + 0x1E1650(unit, 0), unclamped.
    pub defence: i32,
    /// 0x24F7D0(terrain of the cell, unit): 0 none, 1 favoured, 2 not.
    pub terrain_effect: i32,
    /// 0x247E10(unit, col, row): steps in the low nibble, bit 4 = may enter
    /// Labyrinth cells (the HUD tests the low nibble >= 2).
    pub allowance: i32,
}

/// A record resolved against the engine state.
#[derive(Clone, Copy)]
struct Rec<'a> {
    card: &'a CardInfo,
    /// A field unit (groups 0/1), not a hand record.
    field: bool,
    /// +0x44 / +0x45.
    side: u8,
    slot: u8,
    /// +0x57 bits 1–4.
    control_changed: bool,
    face_up: bool,
    defence: bool,
    committed_face_up: bool,
    /// +0x38 / +0x3A.
    attack_bonus: i16,
    defense_bonus: i16,
    /// The cell at +0x40/+0x41 points back at this record.
    on_board: bool,
}

fn other(side: u8) -> u8 {
    u8::from(side == 0)
}

fn class(card: &CardInfo) -> u8 {
    card.kind >> 5
}

fn race(card: &CardInfo) -> u8 {
    card.kind & 0x1F
}

/// The record's +0x08..+0x2F as halfwords: five entries (word0, word1),
/// `None` as the all-ones sentinel.
pub fn entry_halfwords(card: &CardInfo) -> [u16; 20] {
    let mut out = [0xFFFF; 20];
    for (k, entry) in card.entries.iter().enumerate() {
        if let Some([w0, w1]) = entry {
            out[4 * k..4 * k + 4].copy_from_slice(&[
                *w0 as u16,
                (w0 >> 16) as u16,
                *w1 as u16,
                (w1 >> 16) as u16,
            ]);
        }
    }
    out
}

/// 0x1E1F00's case table for ability `a` (0..=16) of a record area
/// (`halfwords`, from +0x08) at rank `rank` (0x2154C0(side, card), −1 for
/// none), without the card and option checks. Abilities 0 and 1 test a
/// non-zero halfword; the others need `rank` >= the requirement byte (the
/// high byte, the low byte for 7, 8, 10 and 14); 0xFFFF is "none".
pub fn record_ability(halfwords: &[u16; 20], a: u8, rank: i32) -> bool {
    // (halfword index from +0x08, requirement in the low byte)
    const CASES: [(usize, bool); 17] = [
        (1, false),
        (0, false),
        (3, false),
        (2, false),
        (5, false),
        (4, false),
        (7, false),
        (6, true),
        (9, true),
        (8, false),
        (11, true),
        (10, false),
        (13, false),
        (12, false),
        (15, true),
        (14, false),
        (17, false),
    ];
    let Some(&(index, low)) = CASES.get(usize::from(a)) else {
        return false;
    };
    let h = halfwords[index];
    if h == 0xFFFF {
        return false;
    }
    if a < 2 {
        return h != 0;
    }
    let requirement = if low { h & 0xFF } else { h >> 8 };
    rank >= i32::from(requirement)
}

/// The abilities (0..=16) a leader's row (table 0x2C26B8 + card·40, copied
/// to the leader record +0x08) grants at `rank`, as the engine keeps them in
/// [`super::SideState::leader_abilities`]. The option-28 bit and the card
/// checks of 0x1E1EF0 are applied by [`Query`].
pub fn leader_abilities(row: &[u16; 20], rank: i32) -> Vec<u8> {
    (0..=16).filter(|&a| record_ability(row, a, rank)).collect()
}

/// 0x24F6A0: side mode 1 (same controller), 2 (the other) or 3 (both).
fn relation(target: u8, source: u8, mode: u32) -> bool {
    match mode {
        3 => true,
        2 => target == other(source),
        1 => target == source,
        _ => false,
    }
}

/// 0x2735C0: the races an ability-14 leader of race `leader` weakens (the
/// jump table 0x2D3460 over `leader & 0x1F` < 0x12; the planner's copy of
/// 0x273420 in `ai::weevil::world` shares it).
pub(crate) fn weakens(leader: i32, target: u8) -> bool {
    match leader & 0x1F {
        1 => target == 7,
        2 => target == 3,
        3 => target < 2,
        5 => target == 12,
        6 => matches!(target, 9 | 12),
        7 => target == 1,
        9 => target == 13,
        11 => matches!(target, 5 | 6),
        12 => target == 15,
        15 => matches!(target, 2 | 14),
        16 => matches!(target, 14 | 17),
        17 => target == 4,
        _ => false,
    }
}

/// The getters over one engine state.
pub struct Query<'a, C: CardData + ?Sized> {
    pub duel: &'a Duel,
    pub cards: &'a C,
    pub ctx: &'a Context,
}

impl<'a, C: CardData + ?Sized> Query<'a, C> {
    pub fn new(duel: &'a Duel, cards: &'a C, ctx: &'a Context) -> Self {
        Self { duel, cards, ctx }
    }

    /// "The other side" of `side`: [`other`] in a 2-player duel; in an
    /// N-player duel (a port addition) the side's opponent
    /// (`duel/multi.rs`).
    fn other(&self, side: u8) -> u8 {
        if self.duel.multi().is_some() {
            self.duel.opp(side)
        } else {
            other(side)
        }
    }

    /// 0x24F6A0 ([`relation`]); in an N-player duel mode 2 means any enemy.
    fn relation(&self, target: u8, source: u8, mode: u32) -> bool {
        if self.duel.multi().is_some() && mode == 2 {
            return self.duel.foe(target, source);
        }
        relation(target, source, mode)
    }

    fn resolve(&self, record: Record<'a>) -> Option<Rec<'a>> {
        Some(match record {
            Record::Field(u) => Rec {
                card: self.cards.card(u.card)?,
                field: true,
                side: u.side,
                slot: u.slot,
                control_changed: u.stolen || self.ctx.control_changed.contains(&u.id),
                face_up: u.face_up,
                defence: u.defence,
                committed_face_up: u.committed_face_up,
                attack_bonus: u.attack_bonus,
                defense_bonus: u.defense_bonus,
                on_board: u.position.index().is_some()
                    && self.duel.unit_at(u.position).is_some_and(|o| o.id == u.id),
            },
            Record::Hand { side, slot, card } => Rec {
                card: self.cards.card(card)?,
                field: false,
                side,
                slot,
                control_changed: false,
                face_up: false,
                defence: false,
                committed_face_up: false,
                attack_bonus: 0,
                defense_bonus: 0,
                on_board: false,
            },
        })
    }

    /// 0x1DE3A0 / 0x1E07C0: the valid field record (group `side`, `slot`).
    fn field(&self, side: u8, slot: u8) -> Option<&'a DuelUnit> {
        self.duel
            .units
            .iter()
            .find(|u| u.side == side && u.slot == slot)
    }

    /// 0x251C80(side, slot) = 0x24F790: `side`, or the other side when the
    /// record there has +0x57 bit 1.
    pub fn controller(&self, side: u8, slot: u8) -> u8 {
        match self.field(side, slot) {
            Some(u) if self.duel.multi().is_some() && u.stolen => u.controller(),
            Some(u) if u.stolen || self.ctx.control_changed.contains(&u.id) => self.other(side),
            _ => side,
        }
    }

    /// 0x251C90: the record's own controller.
    fn own_controller(&self, r: &Rec) -> u8 {
        if r.control_changed && self.duel.multi().is_some() {
            self.controller(r.side, r.slot)
        } else if r.control_changed {
            other(r.side)
        } else {
            r.side
        }
    }

    /// 0x268710 / 0x268590: records of groups 0 then 1, slots 1..=10, that
    /// are committed face-up (+0x57 bit 4), in defence (bit 3) and committed
    /// defence (bit 5) with an entry 2 of type `ty`, in search order.
    fn type_sources(&self, ty: u32) -> Vec<(&'a DuelUnit, [u32; 2])> {
        let mut out = Vec::new();
        for side in 0..self.duel.side_count() {
            for slot in 1..=10 {
                let Some(u) = self.field(side, slot) else {
                    continue;
                };
                if !(u.committed_face_up && u.defence && u.committed_defence) {
                    continue;
                }
                if let Some(e) = self.cards.card(u.card).and_then(|c| c.entries[2]) {
                    if e[0] >> 8 == ty {
                        out.push((u, e));
                    }
                }
            }
        }
        out
    }

    /// 0x268890(0x58, other side) > 0: an opposing type-0x58 source
    /// switches off `side`'s leader abilities in 0x273070.
    fn suppressed(&self, side: u8) -> bool {
        self.type_sources(0x58)
            .iter()
            .any(|(u, _)| self.duel.foe(self.controller(u.side, u.slot), side))
    }

    /// 0x1E1E80(side, 0, n, 0): ability `n` of the leader record of group
    /// `side` (0x1E1EF0 with the engine's evaluated list).
    fn leader_ability(&self, side: u8, n: u8) -> bool {
        let Some(leader) = self.field(side, 0) else {
            return false;
        };
        leader.card < CARD_LIMIT
            && leader.card != NO_RECORD_CARD
            && self.ctx.option_28 & (1u32 << (leader.side & 31)) != 0
            && n <= 16
            && self
                .duel
                .sides
                .get(usize::from(side))
                .is_some_and(|s| s.leader_abilities.contains(&n))
    }

    /// 0x273070(n, side).
    fn leader_has(&self, side: u8, n: u8) -> bool {
        !self.suppressed(side) && self.leader_ability(side, n)
    }

    /// 0x1E1910 / 0x1E1980(side, 0): the leader's kind byte, −1 without one.
    fn leader_kind(&self, side: u8) -> i32 {
        self.field(side, 0)
            .and_then(|u| self.cards.card(u.card))
            .map_or(-1, |c| i32::from(c.kind))
    }

    /// 0x1E1B70(side, 0, 0): clamp(level − leader +0x48, 0, 12), −1
    /// without a leader.
    fn leader_level(&self, side: u8) -> i32 {
        self.field(side, 0)
            .and_then(|u| self.cards.card(u.card).map(|c| (c, u.discount)))
            .map_or(-1, |(c, discount)| {
                (i32::from(c.level) - i32::from(discount)).clamp(0, 12)
            })
    }

    /// 0x273370(side, col, row): the cell lies in the square around the
    /// leader of `side` (half-size 2 with ability 9, else 1; 0x273280).
    fn in_range(&self, side: u8, cell: GridPos) -> bool {
        let Some(leader) = self.field(side, 0) else {
            return false;
        };
        let half = if self.leader_has(side, 9) { 2 } else { 1 };
        let (lc, lr) = (
            i32::from(leader.position.column),
            i32::from(leader.position.row),
        );
        let (c, r) = (i32::from(cell.column), i32::from(cell.row));
        (lc - half).max(0) <= c
            && c <= (lc + half).min(6)
            && (lr - half).max(0) <= r
            && r <= (lr + half).min(6)
    }

    /// 0x273830(cell, side, race, n): in range of `side`'s leader, which has
    /// ability `n` and the same race.
    fn race_aura(&self, cell: GridPos, side: u8, race: u8, n: u8) -> bool {
        self.in_range(side, cell)
            && self.leader_has(side, n)
            && self.leader_kind(side) & 0x1F == i32::from(race)
    }

    /// 0x273420(unit, col, row): ability 10 of the controller's leader
    /// (+100 per leader level, at most 500, for its own race in range) and
    /// ability 14 of the other leader (the same, subtracted, for the races
    /// its race weakens).
    fn leader_aura(&self, r: &Rec, cell: GridPos) -> i32 {
        let ctrl = self.own_controller(r);
        let opp = self.other(ctrl);
        let race = race(r.card);
        let mut sum = 0;
        if self.race_aura(cell, ctrl, race, 10) {
            sum += (self.leader_level(ctrl) * 100).min(500);
        }
        if !self.in_range(opp, cell) {
            return sum;
        }
        if self.leader_has(opp, 14) && weakens(self.leader_kind(opp), race) {
            sum -= (self.leader_level(opp) * 100).min(500);
        }
        sum
    }

    /// 0x273160(side): the summon discount of ability 8 by leader race.
    fn discount(&self, side: u8) -> i32 {
        if !self.leader_has(side, 8) {
            return 0;
        }
        let level = self.leader_level(side);
        match self.leader_kind(side) & 0x1F {
            0x14 => 1,
            0xD | 4 => 1 + i32::from(level >= 6),
            0xE | 0xC | 6 | 0 => 1 + i32::from(level >= 7),
            _ => 0,
        }
    }

    /// 0x2703B0(side, card): the discount for a monster of the leader's race.
    fn summon_discount(&self, side: u8, card: u16) -> i32 {
        if card == NO_RECORD_CARD || card >= CARD_LIMIT {
            return 0;
        }
        let Some(c) = self.cards.card(card) else {
            return 0;
        };
        if i32::from(race(c)) != self.leader_kind(side) & 0x1F || class(c) != 0 {
            return 0;
        }
        self.discount(side)
    }

    /// 0x2505F0(unit, source controller, mode, filter): a stat aura reaches
    /// the monster record `r`.
    fn filter(&self, r: &Rec, source: u8, mode: u32, filter: u32) -> bool {
        let c = r.card;
        if class(c) != 0 {
            return false;
        }
        let side_ok = self.relation(self.own_controller(r), source, mode);
        let kind_ok = match filter {
            0x13 | 0x11 | 0x10 | 7 | 6 | 2 | 0xF | 0xE => u32::from(race(c)) == filter,
            0x19 => c.attribute == 5,
            0x14 => c.attribute == 0,
            0x15 => c.attribute == 1,
            0x31 => power_up_fits(c, 0x2FF),
            0x1D => true,
            _ => false,
        };
        side_ok && kind_ok
    }

    /// 0x1E1570 / 0x1E1650(unit, 0): clamp(base + permanent change, 0, 9999).
    fn base(r: &Rec, attack: bool) -> i32 {
        let (value, bonus) = if attack {
            (r.card.attack, r.attack_bonus)
        } else {
            (r.card.defense, r.defense_bonus)
        };
        (i32::from(value) + i32::from(bonus)).clamp(0, 9999)
    }

    /// 0x1E1510 / 0x1E15F0(side, 0, 1): the leader's raw value, −1 without.
    fn leader_value(&self, side: u8, attack: bool) -> i32 {
        self.field(side, 0)
            .and_then(|u| self.cards.card(u.card))
            .map_or(-1, |c| {
                i32::from(if attack { c.attack } else { c.defense }).clamp(0, 9999)
            })
    }

    /// 0x24F7D0(0x1E3530(col, row), unit).
    fn terrain_effect(&self, r: &Rec, cell: GridPos) -> i32 {
        if class(r.card) != 0 || r.slot == 0 {
            return 0;
        }
        match self
            .duel
            .terrain_of(cell)
            .map(|t| terrain_affinity(t, r.card))
        {
            Some(1) => 1,
            Some(-1) => 2,
            _ => 0,
        }
    }

    /// 0x267A00(unit, col, row, attack): everything but the base value.
    fn modifiers(&self, r: &Rec, cell: GridPos, attack: bool) -> i32 {
        let ctrl = self.own_controller(r);
        let mut sum = 0i32;
        if r.card.id == LEADER_COPY_CARD {
            let (value, bonus) = if attack {
                (r.card.attack, r.attack_bonus)
            } else {
                (r.card.defense, r.defense_bonus)
            };
            sum -= (i32::from(value) + i32::from(bonus)).min(9999);
            let leader = if r.defence { ctrl } else { self.other(ctrl) };
            sum += self.leader_value(leader, attack);
        }
        // Slots 1..=5 of the controller's group, then of the other group.
        for group in [ctrl, self.other(ctrl)] {
            for slot in 1..=5 {
                let Some(src) = self.field(group, slot) else {
                    continue;
                };
                if !(src.committed_face_up && src.defence && src.committed_defence) {
                    continue;
                }
                let Some([w0, w1]) = self.cards.card(src.card).and_then(|c| c.entries[2]) else {
                    continue;
                };
                let source = self.controller(group, slot);
                if !self.filter(r, source, (w0 >> 6) & 3, w0 & 0x3F) {
                    continue;
                }
                let value = (w1 & 0xFFFF) as i32;
                match w0 >> 8 {
                    0x22 if !attack => sum += value,
                    0x12 => sum += value,
                    0x13 => sum -= value,
                    _ => (),
                }
            }
        }
        sum += match self.terrain_effect(r, cell) {
            2 => -500,
            1 => 500,
            _ => 0,
        };
        sum + self.leader_aura(r, cell)
    }

    /// 0x247E10(unit, col, row).
    fn allowance(&self, r: &Rec, cell: GridPos) -> i32 {
        if r.slot == 0 {
            return if self.leader_has(r.side, 4) { 2 } else { 1 };
        }
        let [w0, w1] = r.card.entries[1].unwrap_or([u32::MAX; 2]);
        let movement = w0 >> 8;
        if !r.face_up {
            return match movement {
                0x4F => 0,
                8..=10 => 0x11,
                _ => 1,
            };
        }
        let mut v = match self.terrain_effect(r, cell) {
            1 => 2,
            _ => 1,
        };
        match movement {
            6 => v = (w1 & 0xFFFF) as i32,
            8..=10 => v |= 0x10,
            0x4F => return 0,
            _ => (),
        }
        // Entry-2 type-6 sources, slot by slot, group 0 before group 1;
        // committed face-up and committed defence (bits 4/5) only.
        for slot in 1..=10 {
            for side in 0..self.duel.side_count() {
                let Some(src) = self.field(side, slot) else {
                    continue;
                };
                if !(src.committed_face_up && src.committed_defence) {
                    continue;
                }
                let Some([s0, s1]) = self.cards.card(src.card).and_then(|c| c.entries[2]) else {
                    continue;
                };
                if s0 >> 8 != 6
                    || (s0 & 0x3F == 0xF && !(race(r.card) == 0xF && class(r.card) == 0))
                {
                    continue;
                }
                v = (v & !0xF) | (s1 & 0xFFFF) as i32;
            }
        }
        if !self.type_sources(0x43).is_empty() {
            v = (v & !0xF) | 1;
        }
        if self.race_aura(cell, r.side, race(r.card), 12) {
            v = (v & !0xF) | 2;
        }
        v
    }

    /// 0x268CA0(unit, viewer): shared by [`Self::legend_query`] and
    /// [`Self::hidden`].
    fn legend(&self, r: &Rec, viewer: u8) -> bool {
        let ctrl = self.own_controller(r);
        if viewer == ctrl || r.slot == 0 {
            return false;
        }
        if !r.face_up {
            return true;
        }
        for (src, [w0, _]) in self.type_sources(0x44) {
            if self.controller(src.side, src.slot) != ctrl {
                continue;
            }
            match w0 & 0x3F {
                0x1E => return class(r.card) != 0,
                0x1D => return class(r.card) == 0,
                _ => (),
            }
        }
        false
    }

    /// 0x268CA0(unit, viewer) != 0: a non-leader controlled by the other
    /// side is face-down (+0x57 bit 2 clear), or the first type-0x44 source
    /// of its controller with filter 0x1D (monsters) / 0x1E (other cards)
    /// covers it. The HUD's legend passes viewer = d3c (0x1B3F18), 0x1DA6A0
    /// too; `tools/export_getters.py` recorded viewer = 0.
    pub fn legend_query(&self, record: Record<'a>, viewer: u8) -> bool {
        self.resolve(record)
            .is_some_and(|r| self.legend(&r, viewer))
    }

    /// 0x1DA6A0(unit) with d3c = `viewer` (0 or 1): the card box hides a
    /// face-down card. Records of sides >= 2 are never hidden. With
    /// option 29 set for the record's side, hiding needs a computer viewer
    /// (controller < 32) against a pad side (>= 100), and on the field also
    /// a computer controller; without it a record off the board is hidden
    /// unless R2 is held on the viewer's pad, and a field record controlled
    /// by the other side always. On the field, [`Self::legend_query`] hides
    /// first and a committed face-up record (+0x57 bit 4) never is.
    pub fn hidden(&self, record: Record<'a>, viewer: u8) -> bool {
        let Some(r) = self.resolve(record) else {
            return false;
        };
        let by_type = self.ctx.option_29 & (1u32 << (r.side & 31)) != 0;
        if !r.field && r.side >= self.duel.side_count() {
            return false;
        }
        let multi = self.duel.multi().is_some();
        let d3c = if multi { viewer } else { viewer & 1 };
        let ctl = |side: u8| self.ctx.controllers[usize::from(side & 1)];
        let computer_view = ctl(d3c) < 32 && ctl(if multi { self.other(d3c) } else { d3c ^ 1 }) >= 100;
        let r2 = self.ctx.r2_held[usize::from(d3c & 1)];
        if !r.on_board {
            return if by_type { computer_view } else { !r2 };
        }
        if self.legend(&r, viewer) {
            return true;
        }
        if r.committed_face_up {
            return false;
        }
        let mut ctrl = self.controller(r.side, r.slot);
        if multi && self.duel.team_mates(viewer, ctrl) {
            // 2v2 (a port addition): a team-mate's cards are seen as own.
            ctrl = viewer;
        }
        if by_type {
            computer_view && ctl(ctrl) < 32
        } else {
            viewer != ctrl || !r2
        }
    }

    /// 0x1E1C60(unit, 0) != 0: a leader on its own cell reads the leader
    /// table (always present); any other record has an effect when its card
    /// record's effect index (+4) is set, i.e. the card has an entry.
    pub fn has_effect(&self, record: Record<'a>) -> bool {
        let Some(r) = self.resolve(record) else {
            return false;
        };
        if r.on_board && r.slot == 0 {
            return r.card.id < CARD_LIMIT && r.card.id != NO_RECORD_CARD;
        }
        r.card.entries.iter().any(Option::is_some)
    }

    /// 0x1E1EF0(unit, a, 0): card < 0x356 and not 0x29F, option 28 set for
    /// the record's side, a <= 16; a field leader then has the ability when
    /// its side's [`super::SideState::leader_abilities`] lists it, any other
    /// record when [`record_ability`] finds it in its effect entries at
    /// `rank(side, card)` (0x2154C0, save data: −1 for none).
    pub fn ability(&self, record: Record<'a>, a: u8, rank: impl Fn(u8, u16) -> i32) -> bool {
        let Some(r) = self.resolve(record) else {
            return false;
        };
        let id = r.card.id;
        if id >= CARD_LIMIT || id == NO_RECORD_CARD {
            return false;
        }
        if self.ctx.option_28 & (1u32 << (r.side & 31)) == 0 || a > 16 {
            return false;
        }
        if r.field && r.slot == 0 {
            return self
                .duel
                .sides
                .get(usize::from(r.side))
                .is_some_and(|s| s.leader_abilities.contains(&a));
        }
        record_ability(&entry_halfwords(r.card), a, rank(r.side, id))
    }

    /// The HUD's ability icons: bit `a` for a in 4..=16 with
    /// [`Self::ability`].
    pub fn abilities(&self, record: Record<'a>, rank: impl Fn(u8, u16) -> i32) -> u32 {
        (4..=16)
            .filter(|&a| self.ability(record, a, &rank))
            .fold(0, |bits, a| bits | 1 << a)
    }

    /// The card box values of `record` evaluated at `cell` (the cursor or
    /// the record's own cell); `None` when the card is unknown.
    pub fn stats(&self, record: Record<'a>, cell: GridPos) -> Option<Stats> {
        let r = self.resolve(record)?;
        Some(Stats {
            attack: self.modifiers(&r, cell, true) + Self::base(&r, true),
            defence: self.modifiers(&r, cell, false) + Self::base(&r, false),
            terrain_effect: self.terrain_effect(&r, cell),
            allowance: self.allowance(&r, cell),
        })
    }

    /// 0x2692E0(a.side, a.slot, b.side, b.slot) for field records: −1 when
    /// B's controller is the active side ([0x2DB8F4]), either is missing,
    /// not a monster or a leader, or B is covered by a type-0x44 source
    /// (0x268E80); else 1 + [`attribute_relation`](A, B) (0 A weaker, 1
    /// neutral, 2 A stronger) when B is committed face-up or a type-0x3D
    /// source exists, −1 otherwise.
    pub fn advantage(&self, a: (u8, u8), b: (u8, u8)) -> i32 {
        if self.controller(b.0, b.1) == self.duel.active {
            return -1;
        }
        let (Some(ua), Some(ub)) = (self.field(a.0, a.1), self.field(b.0, b.1)) else {
            return -1;
        };
        let (Some(ca), Some(rb)) = (self.cards.card(ua.card), self.resolve(Record::Field(ub)))
        else {
            return -1;
        };
        if class(ca) != 0 || class(rb.card) != 0 || a.1 == 0 || b.1 == 0 {
            return -1;
        }
        // 0x268E80(B).
        if self.own_controller(&rb) != self.duel.active {
            for (_, [w0, _]) in self.type_sources(0x44) {
                let covered = match w0 & 0x3F {
                    0x1E => class(rb.card) != 0,
                    0x1D => class(rb.card) == 0,
                    _ => continue,
                };
                if covered {
                    return -1;
                }
                break;
            }
        }
        let value = attribute_relation(ca.attribute, rb.card.attribute) + 1;
        if ub.committed_face_up || !self.type_sources(0x3D).is_empty() {
            value
        } else {
            -1
        }
    }

    /// 0x1E1BD0(unit, 0) for a field unit: clamp(level − +0x48, 0, 12) (the
    /// stars of its card image); −1 for an unknown card.
    pub fn unit_cost(&self, unit: &DuelUnit) -> i32 {
        self.cards.card(unit.card).map_or(-1, |c| {
            (i32::from(c.level) - i32::from(unit.discount)).clamp(0, 12)
        })
    }

    /// 0x1E1B70(2, i, 0) for the hand record of `side`'s `card` (the hand
    /// menu's left box): clamp(level − 0x2703B0(side, card), 0, 12); −1 for
    /// an unknown card (a record without a card).
    pub fn hand_card_cost(&self, side: u8, card: u16) -> i32 {
        self.cards.card(card).map_or(-1, |c| {
            (i32::from(c.level) - self.summon_discount(side, card)).clamp(0, 12)
        })
    }

    /// 0x2713A0(card, side): the level, less the ability-8 discount for a
    /// monster of the leader's race; 255 for 0x29F and ids past the table.
    fn card_cost(&self, card: u16, side: u8) -> i32 {
        if card == NO_RECORD_CARD || card >= CARD_LIMIT {
            return 0xFF;
        }
        let Some(c) = self.cards.card(card) else {
            return 0xFF;
        };
        let level = i32::from(c.level);
        if class(c) == 0 && i32::from(race(c)) == self.leader_kind(side) & 0x1F {
            level - self.discount(side)
        } else {
            level
        }
    }

    /// 0x270280(): the highest cost among the active side's marked hand
    /// cards (`marks`: hand indices, e.g. `ui::HandMenu::marks`), at least 0.
    pub fn marked_cost(&self, marks: &[usize]) -> i32 {
        let side = self.duel.active;
        let Some(s) = self.duel.sides.get(usize::from(side)) else {
            return 0;
        };
        s.hand
            .iter()
            .enumerate()
            .filter(|(i, _)| marks.contains(i))
            .map(|(_, &card)| self.card_cost(card, side))
            .fold(0, i32::max)
    }
}

#[cfg(test)]
mod tests {
    //! Synthetic cards and positions (invented numbers, not retail data);
    //! the private `hud_queries_check` compares with the original.
    use super::*;
    use crate::duel::{kind, terrain, SideState};

    fn card(id: u16, kind: u8, level: u8, attribute: u8, attack: u16, defense: u16) -> CardInfo {
        CardInfo {
            id,
            kind,
            level,
            attribute,
            attack,
            defense,
            effects: [false; 5],
            toon: false,
            kind_index: 0,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        }
    }

    fn cards() -> Vec<CardInfo> {
        let mut v: Vec<CardInfo> = (0..8)
            .map(|i| card(i, kind::MACHINE, 3, 0, 1000, 800))
            .collect();
        // Leaders of sides 0 and 1; card 2 is favoured on forest; card 3 a
        // Machine aura, +300 to own Machines (entry 2 type 0x12, mode 1).
        v[0] = card(0, kind::DRAGON, 6, 0, 2000, 1500);
        v[1] = card(1, kind::SPELLCASTER, 4, 1, 1500, 1200);
        v[2] = card(2, kind::BEAST, 4, 3, 1200, 1000);
        v[3].entries[2] = Some([0x1240 | u32::from(kind::MACHINE), 300]);
        // Entry-1 type 6: moves `word1` steps.
        v[4].entries[1] = Some([0x0600, 3]);
        v[5] = card(5, kind::DRAGON, 4, 0, 1600, 1200);
        v
    }

    fn unit(id: u16, card: u16, side: u8, slot: u8, column: u8, row: u8) -> DuelUnit {
        DuelUnit {
            id: UnitId(id),
            card,
            side,
            slot,
            position: GridPos { column, row },
            face_up: true,
            defence: false,
            committed_face_up: true,
            committed_defence: false,
            acted: false,
            spellbind: 0,
            summon_order: 0,
            attack_bonus: 0,
            defense_bonus: 0,
            spellbind_fresh: false,
            evo_counter: 0,
            stolen: false,
            discount: 0,
            stolen_by: None,
        }
    }

    fn side(abilities: Vec<u8>, hand: Vec<u16>) -> SideState {
        SideState {
            lp: 4000,
            stars: 4,
            can_summon: true,
            hand,
            deck: vec![],
            leader_abilities: abilities,
            summons: 0,
            graveyard: vec![],
            destiny: 0,
        }
    }

    fn duel(units: Vec<DuelUnit>, abilities: [Vec<u8>; 2]) -> Duel {
        let mut terrain = vec![terrain::NORMAL; 49];
        terrain[3 * 7 + 2] = terrain::FOREST;
        let [a, b] = abilities;
        Duel::from_observed(
            terrain,
            units,
            [side(a, vec![5, 2, 3]), side(b, vec![])],
            1,
            0,
            0,
            &cards(),
        )
        .unwrap()
    }

    fn leaders() -> Vec<DuelUnit> {
        vec![unit(0, 0, 0, 0, 3, 6), unit(1, 1, 1, 0, 3, 0)]
    }

    fn ctx() -> Context {
        Context {
            option_28: 3,
            option_29: 3,
            controllers: [100, 2],
            ..Context::default()
        }
    }

    #[test]
    fn ability_rows_decode_like_the_case_table() {
        let mut row = [0xFFFF; 20];
        assert!(!record_ability(&row, 4, 12));
        row[5] = 0x03FF; // ability 4: rank 3 or more (high byte)
        row[11] = 0x0502; // ability 10: rank 2 or more (low byte)
        row[0] = 0x0001; // ability 1: any non-zero halfword
        assert!(record_ability(&row, 4, 3) && !record_ability(&row, 4, 2));
        assert!(record_ability(&row, 10, 2) && !record_ability(&row, 10, 1));
        assert!(record_ability(&row, 1, -1));
        assert!(!record_ability(&row, 17, 12));
        assert_eq!(leader_abilities(&row, 3), vec![1, 4, 10]);
    }

    #[test]
    fn stats_follow_the_cell_argument() {
        let mut units = leaders();
        units.push(unit(2, 2, 0, 1, 2, 3)); // beast on the forest cell
        let d = duel(units, [vec![], vec![]]);
        let (cards, ctx) = (cards(), ctx());
        let q = Query::new(&d, cards.as_slice(), &ctx);
        let beast = Record::Field(d.unit(UnitId(2)).unwrap());
        let on_forest = q.stats(beast, GridPos { column: 2, row: 3 }).unwrap();
        let elsewhere = q.stats(beast, GridPos { column: 4, row: 4 }).unwrap();
        assert_eq!(
            on_forest,
            Stats {
                attack: 1700,
                defence: 1500,
                terrain_effect: 1,
                allowance: 2
            }
        );
        assert_eq!(
            elsewhere,
            Stats {
                attack: 1200,
                defence: 1000,
                terrain_effect: 0,
                allowance: 1
            }
        );
        // Off the board: no terrain, no favour.
        let off = q.stats(beast, GridPos { column: 9, row: 9 }).unwrap();
        assert_eq!((off.attack, off.allowance), (1200, 1));
    }

    #[test]
    fn auras_movement_and_leader_abilities() {
        let mut units = leaders();
        let mut source = unit(2, 3, 0, 1, 0, 5);
        (source.defence, source.committed_defence) = (true, true);
        units.push(source);
        units.push(unit(3, 6, 0, 2, 1, 5)); // own Machine
        units.push(unit(4, 7, 1, 1, 1, 1)); // enemy Machine
        units.push(unit(5, 4, 0, 3, 5, 5)); // entry-1 type 6
        units.push(unit(6, 5, 0, 4, 3, 5)); // dragon beside its dragon leader
        let d = duel(units, [vec![4, 10, 12], vec![]]);
        let (cards, ctx) = (cards(), ctx());
        let q = Query::new(&d, cards.as_slice(), &ctx);
        let at = |id: u16| {
            let u = d.unit(UnitId(id)).unwrap();
            q.stats(Record::Field(u), u.position).unwrap()
        };
        // +300 from the own aura, for the source too; mode 1: not the
        // enemy's. Entry-1 type 6 replaces the steps. Ability 4: the leader
        // walks 2 steps. Ability 10: +100 per leader level (6), at most 500;
        // ability 12: 2 steps.
        assert_eq!(at(3).attack, 1300);
        assert_eq!(at(4).attack, 1000);
        assert_eq!(at(2).attack, 1300);
        assert_eq!(at(5).allowance, 3);
        assert_eq!(at(0).allowance, 2);
        assert_eq!((at(6).attack, at(6).allowance), (2100, 2));
        let far = q
            .stats(
                Record::Field(d.unit(UnitId(6)).unwrap()),
                GridPos { column: 0, row: 0 },
            )
            .unwrap();
        assert_eq!((far.attack, far.allowance), (1600, 1));
        assert_eq!(
            q.abilities(Record::Field(d.unit(UnitId(0)).unwrap()), |_, _| 0),
            1 << 4 | 1 << 10 | 1 << 12
        );
    }

    #[test]
    fn hidden_and_legend_follow_controllers_and_options() {
        let mut units = leaders();
        let mut own = unit(2, 2, 0, 1, 2, 5);
        (own.face_up, own.committed_face_up) = (false, false);
        let mut enemy = unit(3, 2, 1, 1, 2, 1);
        (enemy.face_up, enemy.committed_face_up) = (false, false);
        units.extend([own, enemy]);
        let d = duel(units, [vec![], vec![]]);
        let cards = cards();
        let mut ctx = ctx();
        let own = Record::Field(d.unit(UnitId(2)).unwrap());
        let enemy = Record::Field(d.unit(UnitId(3)).unwrap());
        let hand = Record::Hand {
            side: 0,
            slot: 1,
            card: 2,
        };
        {
            let q = Query::new(&d, cards.as_slice(), &ctx);
            assert!(q.legend_query(enemy, 0) && !q.legend_query(own, 0));
            assert!(q.hidden(enemy, 0) && !q.hidden(own, 0) && !q.hidden(hand, 0));
        }
        // Option 29 off for side 0: its own face-down card needs R2 held.
        ctx.option_29 = 2;
        let q = Query::new(&d, cards.as_slice(), &ctx);
        assert!(q.hidden(own, 0) && q.hidden(hand, 0));
        ctx.r2_held = [true, false];
        let q = Query::new(&d, cards.as_slice(), &ctx);
        assert!(!q.hidden(own, 0) && !q.hidden(hand, 0));
        // A control change moves the unit to the viewer's side.
        ctx.control_changed = vec![UnitId(3)];
        let q = Query::new(&d, cards.as_slice(), &ctx);
        assert_eq!(q.controller(1, 1), 0);
        assert!(!q.legend_query(enemy, 0));
    }

    #[test]
    fn advantage_and_hand_costs() {
        let mut units = leaders();
        units.push(unit(2, 2, 0, 1, 2, 5)); // earth
        let mut target = unit(3, 5, 1, 1, 2, 1); // light
        target.committed_face_up = false;
        units.push(target);
        let d = duel(units, [vec![8], vec![]]);
        let (cards, ctx) = (cards(), ctx());
        let q = Query::new(&d, cards.as_slice(), &ctx);
        // B face-down and no type-0x3D source: not shown.
        assert_eq!(q.advantage((0, 1), (1, 1)), -1);
        assert_eq!(q.advantage((1, 1), (0, 1)), -1); // B on the active side
                                                     // Ability 8 with a Dragon leader of level 6: Dragons cost one less.
        assert_eq!(q.hand_card_cost(0, 5), 3);
        assert_eq!(q.hand_card_cost(0, 2), 4);
        assert_eq!(q.marked_cost(&[0, 1]), 4);
        assert_eq!(q.marked_cost(&[0]), 3);
        assert_eq!(q.marked_cost(&[]), 0);
    }
}
