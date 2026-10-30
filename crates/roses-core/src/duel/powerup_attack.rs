//! Attacking a power-up: PAL 0x2495AC..0x2497B0, event 0x24 at
//! 0x26AEF0, and the original equip setters at 0x249D80. The ordinary
//! field-equip event has a different trap gate from summon-list equips.

use super::*;

impl Duel {
    /// 0x249D80: category 0 calls capped permanent stat setters; category
    /// 1 writes effect entries (event 0x24 is built for it too: 0x24A410
    /// gives 0x218 / 0x21C, not Riryoku's 0x219); category 2 clears permanent deltas and binding (0x24A024); category 3
    /// permanently binds the monster and uses its action (0x24A070).
    pub(super) fn apply_ordinary_field_equip(
        &mut self,
        id: UnitId,
        category: u8,
        value: u16,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        if category == 1 {
            // 0x249F3C: entry writes on the record (Sword of Dragon's Soul
            // and its siblings); no stat change, no exposure of its own.
            self.engrave_unit(id, value, cards);
        } else if category == 2 {
            // 0x24A024 resets both deltas, +49 and +4A; it preserves the
            // fresh-bind flag, acted state and +4B evolution counter. The
            // separate +4A lock is implicitly zero in supported states:
            // its nonzero writers (event0 mode1 and ability15 event71)
            // are explicitly refused before any state change.
            self.zero_bonuses(id, events);
            self.unbind(id, events);
            // 0x24A060: 0x1E2860(g, s, 0), +0x4A cleared too.
            self.chain_bound.remove(&id.0);
        } else if category == 3 {
            self.bind(id, 0xFF, events);
            if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                u.acted = true;
            }
        } else {
            // Active type-0x34 modifiers remain refused by the continuous
            // effect guard until that separate family is ported.
            // 0x249E40: value + the type-0x34 modifier sum (0x24A7C0).
            let value = i32::from(value) + self.equip_modifier(cards);
            self.boost_capped(id, value, cards, events);
        }
    }

    /// Event 0x24 checks Reverse Trap only for category 0. Its Decree
    /// search is nested inside that reaction. Goblin Fan belongs to
    /// Riryoku's separate event 0x25, which this path still refuses.
    pub(super) fn ordinary_field_equip_guard(
        &self,
        category: u8,
        cards: &impl CardData,
    ) -> Result<()> {
        if category == 0 && self.living_sides().into_iter().any(|g| self.far_trap(g, 0x2B, cards).is_some()) {
            return Err(unsupported("Reverse Trap reacting to a field equip"));
        }
        Ok(())
    }

    pub(super) fn attack_power_up(
        &mut self,
        att: UnitId,
        power: UnitId,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let attacker = self.unit(att).expect("attacker").clone();
        let acard = self.unit_card(&attacker, cards);
        let pcard = cards
            .card(self.unit(power).expect("power-up").card)
            .expect("validated card");
        // 0x2495D4 precedes immunity and compatibility. Ownership, not
        // the temporarily changed controller, selects the experience row.
        self.record_experience(attacker.side, attacker.card, 6);
        if attacker.slot != 0
            // 0x2495E4: leader ability #11 (0x273770) before 0x269020.
            && !self.ability11(&attacker, cards)
            && !self.dragon_immunity(&attacker, cards)
            && power_up_fits(&acard, pcard.id)
        {
            let data = pcard.power_up.ok_or_else(|| {
                unsupported(format!("power-up {} without imported data", pcard.id))
            })?;
            if data.category == 0 && data.value == 0 {
                // Riryoku: event 0x25 (white_rules.rs).
                self.riryoku_attacked(att, power, cards, events)?;
                self.discard(power, events);
                return if self.unit(att).is_some_and(|u| u.spellbind == 0) {
                    self.advance_into(att, cell, cards, events)
                } else {
                    self.crush_sweep(cards, events)
                };
            }
            if data.category == 4 {
                // Event 0x2B: the attacker becomes the result on the
                // power-up's cell (sweep_rules.rs); no advance.
                return self.transform_by_power_up(att, power, cell, true, cards, events);
            }
            if !matches!(data.category, 0..=3) {
                return Err(unsupported(format!(
                    "power-up {} category {} when attacked",
                    pcard.id, data.category
                )));
            }
            self.ordinary_field_equip_guard(data.category, cards)?;
            // The builder exposes the power-up. Event 0x24 commits an
            // already exposed monster, temporarily shows a set monster,
            // and later restores a monster not committed face-up. This
            // exposure does not fire the monster's flip effect.
            if let Some(u) = self.units.iter_mut().find(|u| u.id == att) {
                u.committed_face_up |= u.face_up;
                u.face_up = u.committed_face_up;
            }
            self.apply_ordinary_field_equip(att, data.category, data.value, cards, events);
        }
        // Immune and incompatible monsters destroy the power-up without
        // equipping or exposing it. All removals use its physical owner.
        self.discard(power, events);
        // 0x24BE0C selects 0x20B. Step 0xB's 0x24C66C test suppresses
        // advance after Paralyzing Potion, then runs step 0xE's crush sweep.
        if self.unit(att).is_some_and(|u| u.spellbind == 0) {
            self.advance_into(att, cell, cards, events)
        } else {
            self.crush_sweep(cards, events)
        }
    }
}

#[cfg(test)]
mod tests;
