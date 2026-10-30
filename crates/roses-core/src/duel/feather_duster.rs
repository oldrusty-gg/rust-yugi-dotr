//! Harpie's Feather Duster (magic type 4, word1 >> 16 = 0x12, id 0x2BE) and
//! the one trap that reads its event, Fake Trap (type 0x4B). Builder
//! 0x254F3C, event 7 handler 0x25A080; private notes and interpreter runs in
//! `claude-effects/TRAPS-0x23-0x31-FAKE-TRAP-FIELD.md` §3 (emu F0-F9).

use super::*;

impl Duel {
    /// Called after the card was revealed and discarded (0x25503C removes
    /// it before the event; the cross mask was taken before and never holds
    /// the card itself).
    ///
    /// 1. Mask (0x24E360 with t1 = 0x12, 0x24E974..0x24EAD0): non-leader
    ///    units in slots 1..10 sharing the card's column OR row, controller
    ///    by `mode` (1 own, 2 enemy, 3 both) and the card filter (0x1E: non-
    ///    monster cards). Empty: the builder returns 0, no event, nothing
    ///    else happens (a Fake Trap stays; emu F6).
    /// 2. Event 7 init: 0x273B40(mask, controller) drops units the immunity
    ///    keeps from a non-controller (Lord of D. here; ability #11 is
    ///    refused at setup). Then the oldest Fake Trap of the OTHER group
    ///    (0x26C4B0(ctrl, 0x4B), mask 2), wherever it stands and face-up or
    ///    not (emu F4, F5, F7; an own one is ignored, F3). With one, a
    ///    Royal Decree is searched in the other group first, then the own
    ///    (mask 3 from ctrl ^ 1): a Decree is revealed and marked acted, stays,
    ///    and the destruction goes ahead (F2, F2b); without one the Fake Trap
    ///    is revealed, marked acted and destroyed (0x250C90, stage 0x23) and
    ///    nothing else happens, own targets included (F1, F8).
    /// 3. Otherwise every unit of the filtered mask goes through 0x250C90,
    ///    group 0 then group 1, by slot (bit 21 - i, 0x25A47C; emu F9, F2).
    pub(super) fn feather_duster(
        &mut self,
        card: &DuelUnit,
        mode: u32,
        filter: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let ctrl = card.controller();
        let Some(targets) = self.cross_targets(card, mode, filter, cards) else {
            return Ok(());
        };
        if let Some(fake) = self.far_trap_in(&self.foes(ctrl), trap_type::FAKE_TRAP, cards) {
            if let Some(decree) =
                self.far_trap_in(&self.foes_then_all(ctrl), trap_type::ROYAL_DECREE, cards)
            {
                self.react_trap(decree, None, events);
            } else {
                self.react_trap(fake, None, events);
                self.discard(fake, events);
                return Ok(());
            }
        }
        for t in targets {
            self.remove_by_effect(t, cards, events)?;
        }
        Ok(())
    }

    /// The cross mask of a type-4 / hi-0x12 magic card (0x254F3C: 0x24E360
    /// with t1 = 0x12, 0x24E974..0x24EAD0): non-leader units in slots 1..10
    /// sharing the card's column OR row, controller by `mode` (1 own, 2
    /// enemy, 3 both) and the card filter, minus the card; `None` when it is
    /// empty. The rest is the event's 0x273B40(mask, controller): units the
    /// immunity keeps from a non-controller are dropped (Lord of D.; ability
    /// #11 is refused at setup). Sorted group 0 then 1, by slot (bit 21 - i).
    fn cross_targets(
        &self,
        card: &DuelUnit,
        mode: u32,
        filter: u32,
        cards: &impl CardData,
    ) -> Option<Vec<UnitId>> {
        let ctrl = card.controller();
        let at = card.position;
        let mut mask: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && u.id != card.id
                    && (u.position.column == at.column || u.position.row == at.row)
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        _ => true,
                    }
                    && cards
                        .card(u.card)
                        .is_some_and(|c| select_filter(filter, c).unwrap_or(false))
            })
            .collect();
        if mask.is_empty() {
            return None;
        }
        mask.sort_by_key(|u| (u.side, u.slot));
        Some(
            mask.into_iter()
                .filter(|u| self.magic_target_admitted(u, ctrl, cards))
                .map(|u| u.id)
                .collect(),
        )
    }

    /// Raigeki 700 (0x2BC; type 4, mode 3, filter 0x1D, hi 0x12). The
    /// builder (0x254FDC) writes event 5 {5, card cell, cross mask, ctrl |
    /// ctrl << 16} and returns 1 even for an empty mask (emu RG6); the card
    /// is already in the graveyard. Handler 0x2593C0 (E2-536-TRAP-0x4.md §3):
    /// 1. init: 0x273B40(mask, ctrl); then an Anti-Raigeki (type 0x50) is
    ///    searched with 0x26C4B0(ctrl ^ 1, 0x50), mask 3: the opponent's
    ///    group and the activator's own, oldest summon order (emu RG3, RG4).
    ///    With one, a Royal Decree is searched with 0x26C4B0(ctrl, 0x2E)
    ///    (mask 3, the activator's group first): the Decree is revealed,
    ///    marked acted, stays, and Raigeki goes on (RG5). Without a Decree
    ///    the Anti-Raigeki is revealed and marked acted (0x259A64), STAYS,
    ///    and nothing is destroyed (stage 0x82 only animates; RG3/RG4/RG6).
    /// 2. Otherwise lightning sub-scripts walk out from the card's cell; once
    ///    they are all done every unit of the filtered mask goes through
    ///    0x250C90 (0x25992C, bit 21 - i: group 0 then 1, by slot; no
    ///    entry-4 effect), own monsters included (RG0, RG1, RG2); set cards
    ///    are not monsters (RG7); an enemy Dragon under Lord of D. stays (RG8).
    pub(super) fn raigeki(
        &mut self,
        card: &DuelUnit,
        mode: u32,
        filter: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let ctrl = card.controller();
        let targets = self
            .cross_targets(card, mode, filter, cards)
            .unwrap_or_default();
        if let Some(anti) = self.far_trap_in(&self.foes_then_all(ctrl), trap_type::ANTI_RAIGEKI, cards) {
            if !self.royal_decree_reacts(ctrl, cards, events) {
                self.react_trap(anti, None, events);
                return Ok(());
            }
        }
        for t in targets {
            self.remove_by_effect(t, cards, events)?;
        }
        Ok(())
    }
}
