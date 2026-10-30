//! The event-specific countertraps of PAL 0x258C60 and 0x2593C0.
//! Private original-instruction probes: `codex-seto-effects/countertraps`.

use super::*;

impl Duel {
    /// Event 4 (Dark Hole), 0x258CD8..0x258ED8. Both searches inspect both
    /// groups; the second reverses only the tie-breaking group order. A
    /// White Hole removes its controller's entire group from the target
    /// mask. It is not consumed: the apparent 0x10000 tag is stored with
    /// SH at 0x258DB8/0x258EB4, and the handler never reads a destroy tag.
    ///
    /// Only the second reaction gets the acted bit (0x259070). With equal
    /// ages the two searches can choose different traps: both reveal but
    /// the first keeps its original acted flag. [F, emu]
    pub(super) fn dark_hole_countertraps(
        &mut self,
        controller: u8,
        targets: &mut Vec<UnitId>,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        for (index, first) in self.first_then_all(controller).into_iter().enumerate() {
            let Some(white) = self.far_trap_in(&self.first_then_all(first), 0x2D, cards) else {
                continue;
            };
            let reacting = if let Some(decree) =
                self.far_trap_in(&self.first_then_all(controller), 0x2E, cards)
            {
                decree
            } else {
                let protected = self.unit(white).expect("selected White Hole").controller();
                targets.retain(|&id| self.unit(id).is_some_and(|u| u.side != protected));
                white
            };
            // Reveal both searches, even when they selected the same unit.
            self.activate(reacting, None, events);
            if index >= 1 {
                self.units
                    .iter_mut()
                    .find(|u| u.id == reacting)
                    .expect("reacting trap")
                    .acted = true;
            }
        }
    }
}

#[cfg(test)]
mod tests;
