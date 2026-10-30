//! Continuous Dragon protection, from PAL 0x269020 and the target-filter
//! callers 0x273B40/0x273C60. This is a query over current unit records;
//! there is no stored immunity flag and no radius or controller restriction
//! on the source. Retail effect entries are supplied by `CardData`.

use super::{CardData, Duel, DuelUnit};

impl Duel {
    /// 0x269020: a non-leader Dragon is protected while at least one unit
    /// in either group's slots 1..10 supplies entry-2 type 0xB, filter 0,
    /// with flags 4, 3 and 5 set. Current face (flag 2), controller, acted
    /// state, spellbinding, range, mode bits and the parameter are ignored.
    pub(super) fn dragon_immunity(&self, unit: &DuelUnit, cards: &impl CardData) -> bool {
        unit.slot != 0
            // The record's kind byte (+0x3E), as rewritten by flip type 0x36.
            && self.unit_card(unit, cards).kind == 0
            && self.units.iter().any(|source| {
                source.side < 2
                    && (1..=10).contains(&source.slot)
                    && source.committed_face_up
                    && source.defence
                    && source.committed_defence
                    && cards
                        .card(source.card)
                        .and_then(|c| c.entries[2])
                        .is_some_and(|[word, _]| word >> 8 == 0xB && word & 0x3F == 0)
            })
    }

    /// 0x273B40/0x273C60: a candidate controlled by the other side is
    /// dropped under leader ability #11 (0x273770, tested first, with the
    /// resist box 0x10F) or the 0x269020 immunity (message 0x53).
    pub(super) fn magic_target_admitted(
        &self,
        unit: &DuelUnit,
        by: u8,
        cards: &impl CardData,
    ) -> bool {
        unit.controller() == by
            || !(self.ability11(unit, cards) || self.dragon_immunity(unit, cards))
    }

    /// 0x273770(g, s, 0xB): leader ability #11 protects the unit: its own
    /// cell lies in its controller's leader window (0x273370), that leader
    /// has #11 (0x273070; Moisture Creature suppresses it) and the unit's
    /// record kind & 0x1F equals the leader's (0x1E1980 / 0x1E1910; a
    /// non-monster's kind byte & 0x1F is 0, the Dragon value). No slot,
    /// face or class test.
    pub(super) fn ability11(&self, unit: &DuelUnit, cards: &impl CardData) -> bool {
        self.ability11_at(unit, unit.position, cards)
    }

    /// 0x2738C0(g, s, col, row, 0xB): the same test at a given cell.
    pub(super) fn ability11_at(
        &self,
        unit: &DuelUnit,
        cell: crate::GridPos,
        cards: &impl CardData,
    ) -> bool {
        let kind = self.unit_card(unit, cards).kind;
        self.leader_kind_aura(cell, unit.controller(), kind, 11, cards)
    }
}

#[cfg(test)]
mod tests;
