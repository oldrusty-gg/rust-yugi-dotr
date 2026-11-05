//! Widespread Ruin 812 (limited-range trap, kind index 11; event 0x2E,
//! handler 0x2671B0) and the "Elimination" magic cards of the type-4 / hi-0
//! builder's default branch (Warrior Elimination 720 and kin; event 0x15,
//! handler 0x25D770). Private notes and interpreter runs:
//! the project notes (emu WR0-WR6, E1-E3).

use super::*;

impl Duel {
    /// Event 0x2E {0x2E, trap, mover, 0} (built by 0x24DA54 after Royal
    /// Decree, ability #11 and the mover's 0x269020 immunity):
    /// 1. init: the trap is revealed (0x252FF0); stage 1 marks it acted.
    /// 2. The list: the trap itself first, then every unit on the eight cells
    ///    around the TRAP's cell (column outer, row inner: 0x267468, both
    ///    loops -1..1, the centre skipped), any class, face, position or
    ///    side. A unit controlled by the trap's controller is always taken;
    ///    any other one is dropped under ability #11 (refused at setup) or
    ///    the 0x269020 immunity (emu WR5: an own Red-Eyes under the own Lord
    ///    of D. stays).
    /// 3. Stage 3 (0x26786C): 0x250C90 on every listed unit with a slot
    ///    other than 0 (leaders stay, emu WR6), in list order, then
    ///    0x250C90(rec[8]): the mover, if it is still there.
    ///
    /// The per-cell sub-scripts (0x222) only animate. [F; emu WR1-WR6]
    pub(super) fn widespread_ruin(
        &mut self,
        trap: UnitId,
        mover: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let t = self.unit(trap).expect("trap").clone();
        let ctrl = t.controller();
        let at = t.position;
        let mut around: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.id != trap
                    && u.position != at
                    && u.position.column.abs_diff(at.column) <= 1
                    && u.position.row.abs_diff(at.row) <= 1
                    && self.magic_target_admitted(u, ctrl, cards)
            })
            .collect();
        around.sort_by_key(|u| (u.position.column, u.position.row));
        let victims: Vec<UnitId> = around
            .into_iter()
            .filter(|u| u.slot != 0)
            .map(|u| u.id)
            .collect();
        self.activate(trap, Some(mover), events);
        if let Some(u) = self.units.iter_mut().find(|u| u.id == trap) {
            u.acted = true;
        }
        self.discard(trap, events);
        for v in victims {
            if self.unit(v).is_some() {
                self.remove_by_effect(v, cards, events)?;
            }
        }
        if self.unit(mover).is_some() {
            self.remove_by_effect(mover, cards, events)?;
        }
        Ok(())
    }

    /// Magic type 4, hi 0, an id of [`ELIMINATIONS`] (builder 0x2550A0,
    /// default branch 0x255408), called after the card was revealed and
    /// discarded: mask = 0x24E360(0, 0, ctrl, mode, filter, 0xE) (slots
    /// 1..10 of both groups, controller by `mode`, the card filter) minus
    /// the card, then 0x273C60(mask, ctrl) (the immunity keeps enemy-
    /// controlled Dragons; ability #11 is refused at setup). Record
    /// {0x15, 0, mask, 0}; an empty mask is {0xFE, ...} (nothing, the card is
    /// spent; emu E2). Handler 0x25D770 → 0x260F90: every unit of the mask
    /// through 0x250C90, group 0 then 1, by slot (emu E1: both sides'
    /// Warriors, any face). No countertrap reads event 0x15 (unlike Dark
    /// Hole's event 4). The presentation 0x260D20 draws one rand() for the
    /// camera (0x25051C); not consumed here, like Mirror Force's.
    pub(super) fn elimination(
        &mut self,
        card: &DuelUnit,
        mode: u32,
        filter: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let ctrl = card.controller();
        if filter == 0x20 {
            // 726 Fissure: the single weakest monster (0x24F324), then the
            // immunity filter; a protected pick leaves the mask empty
            // (0xFE: nothing), no runner-up.
            let pick = self.weakest_monster(ctrl, mode, cards).filter(|&t| {
                self.magic_target_admitted(self.unit(t).expect("pick"), ctrl, cards)
            });
            if let Some(t) = pick {
                self.remove_by_effect(t, cards, events)?;
            }
            return Ok(());
        }
        let mut mask: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && u.id != card.id
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        _ => true,
                    }
                    && select_filter(filter, &self.unit_card(u, cards)).unwrap_or(false)
                    && self.magic_target_admitted(u, ctrl, cards)
            })
            .collect();
        mask.sort_by_key(|u| (u.side, u.slot));
        let targets: Vec<UnitId> = mask.into_iter().map(|u| u.id).collect();
        for t in targets {
            self.remove_by_effect(t, cards, events)?;
        }
        Ok(())
    }
}
