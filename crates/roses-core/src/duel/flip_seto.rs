//! Flip families checked against PAL 0x265250 and its event handlers.
//! Private instruction probes: `codex-seto-effects/flips/`.

use super::*;

impl Duel {
    /// [F] 0x265614 builds a controller-relative nonleader mask. Event 0,
    /// 0x257C8C, applies the low byte of the amount and marks the targets
    /// acted whenever the full low halfword is nonzero. Filter 0x1F is the
    /// traced all-card case (Dragon Piper and Invitation to a Dark Sleep).
    pub(super) fn flip_bind_all(
        &mut self,
        id: UnitId,
        low: u32,
        word1: u32,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let source = self.unit(id).expect("flipping unit");
        if low & 0x3F != 0x1F {
            return Err(unsupported(format!("flip effect of card {}", source.card)));
        }
        let (controller, card, mode) = (source.controller(), source.card, (low >> 6) & 3);
        let mut targets: Vec<_> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && match mode {
                        1 => u.controller() == controller,
                        2 => self.foe(u.controller(), controller),
                        3 => true,
                        _ => false,
                    }
            })
            .map(|u| (u.side, u.slot, u.id))
            .collect();
        targets.sort_by_key(|&(side, slot, _)| (side, slot));
        if targets.is_empty() {
            return Ok(());
        }
        events.push(DuelEvent::Activated {
            unit: id,
            card,
            target: None,
        });
        for (_, _, target) in targets {
            let turns = word1 as u8;
            if turns == 0 {
                self.unbind(target, events);
            } else {
                self.bind(target, turns, events);
            }
            if word1 & 0xFFFF != 0 {
                self.units
                    .iter_mut()
                    .find(|u| u.id == target)
                    .unwrap()
                    .acted = true;
            }
        }
        Ok(())
    }

    /// [F] Type 0x38, 0x265E2C: create the parameter card next to the
    /// controller's leader. The first available cell is tested with BLEZ,
    /// so (0,0) prevents activation instead of falling through to another
    /// cell. Event 1 (0x2586A0) creates a fresh unit, then bit16 reveals it
    /// without running its flip. Source ids 0x112/0x129 omit that bit.
    pub(super) fn flip_summon_neighbour(
        &mut self,
        id: UnitId,
        word1: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        if word1 >> 16 != 0x1F {
            return Ok(());
        }
        let source = self.unit(id).expect("flipping unit");
        let (side, source_card) = (source.controller(), source.card);
        let Some(cell) = self.leader_neighbour_cell(side, true) else {
            return Ok(());
        };
        if cell.column == 0 && cell.row == 0 {
            return Ok(());
        }
        if self
            .units
            .iter()
            .filter(|u| u.side == side && (1..=MONSTER_SLOTS as u8).contains(&u.slot))
            .count()
            >= MONSTER_SLOTS
        {
            return Ok(());
        }
        let card = word1 as u16;
        let monster = cards
            .card(card)
            .ok_or_else(|| unsupported(format!("flip summon card {card}")))?
            .is_monster();
        events.push(DuelEvent::Activated {
            unit: id,
            card: source_card,
            target: None,
        });
        if let Some(created) = self.create(side, card, cell, cards, events) {
            if !monster {
                self.units
                    .iter_mut()
                    .find(|u| u.id == created)
                    .unwrap()
                    .acted = true;
            }
            if !matches!(source_card, 0x112 | 0x129) {
                self.expose(created, events);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
