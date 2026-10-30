//! Entry-4 type 0x41, checked against PAL 0x253928 / event 0x3B.
//! Private original-instruction comparisons: `codex-seto-next/destruction/`.

use super::*;

impl Duel {
    /// [F] The threshold reads the physical owner's LP; selection and payment
    /// use the current controller. The threshold is the high half (the boost),
    /// while the cost is the low half, modified by the ordinary loss sources.
    pub(super) fn destruction_boost_for_lp(
        &mut self,
        unit: &DuelUnit,
        word0: u32,
        word1: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Aftermath> {
        let amount = word1 >> 16;
        if u32::from(self.sides[usize::from(unit.side)].lp) <= amount {
            return Ok(Aftermath::Continue);
        }
        // The monster filter is the source-proven Hourglass family. Keep
        // other selection families explicit until their callers are traced.
        if word0 & 0x3F != 0x1D {
            return Err(unsupported(format!(
                "destruction effect of card {}",
                unit.card
            )));
        }
        let controller = unit.controller();
        let mode = (word0 >> 6) & 3;
        let mut targets: Vec<_> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && u.id != unit.id
                    && cards.card(u.card).is_some_and(|c| c.is_monster())
                    && match mode {
                        1 => u.controller() == controller,
                        2 => self.foe(u.controller(), controller),
                        3 => true,
                        _ => false,
                    }
            })
            .map(|u| ((u.side, u.slot), u.id))
            .collect();
        targets.sort_by_key(|(order, _)| *order);
        if targets.is_empty() {
            return Ok(Aftermath::Continue);
        }
        events.push(DuelEvent::Activated {
            unit: unit.id,
            card: unit.card,
            target: None,
        });
        // [F] 0x26AA0C: no reflection, trap negation, leader shield or
        // affordability recheck after the modifier. A modified cost can kill.
        let loss = self.lp_modifier(0x2C, word1 as u16, cards);
        self.damage(controller, loss, events);
        for (_, target) in targets {
            // [F] 0x26AAF8: DEF then ATK, both permanent and capped at 9999.
            // Face, position, binding and acted flags are untouched.
            self.boost_capped(target, amount as i32, cards, events);
        }
        Ok(Aftermath::Continue)
    }
}

#[cfg(test)]
mod tests;
