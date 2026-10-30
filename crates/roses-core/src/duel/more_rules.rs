//! Mask of Darkness 305 / Magician of Faith 43 (flip type 0x14, a graveyard
//! pick), Dokurorider 128 (entry-1 type 2, terrain on entry), Call of the
//! Haunted 745 (magic type 0x36, a kind rewrite) and Fissure 726 (magic type
//! 4 / hi 0, filter 0x20 = the weakest monster). Private notes:
//! `claude-effects/MORE-RULES-305-128-745-726.md`.

use super::*;

impl Duel {
    /// Flip type 0x14 (0x2656B0; 305 Mask of Darkness mode 1, 43 Magician of
    /// Faith mode 3, both filter 0x1E, word1 0):
    /// 1. Nothing unless the flipping unit's controller (0x251C80) is the
    ///    active side (0x1DF7C0 = 0x2DB8F4), in either role.
    /// 2. 0x26C4B0(g ^ 1, 0x4C): a type-0x4C trap answers with event 0x66
    ///    (or, with a Royal Decree, the 0x26CD60 Decree record and result
    ///    4): refused, as for Golgoil.
    /// 3. 0x24FB40(ctrl, 1) < 0 (no free non-Labyrinth neighbour of the
    ///    controller's leader): nothing. Else rec = {1, ctrl | 0x20000,
    ///    that cell, 0}.
    /// 4. Graveyards by mode: 1 the controller's (s7 = ctrl), 3 both (s7 =
    ///    2); mode 2 leaves s7 stale (refused); any filter but 0x1E returns
    ///    0. The non-monster count 0x252260(g, 1..5) of those graveyards is
    ///    0: nothing.
    /// 5. Result 2 with bytes {0, s7 << 4 | 2}: state 0x2B lists the
    ///    graveyard cards with ids >= 0x2AB (0x251920 class code 2), locked
    ///    to one list when s7 < 2; the pick goes into rec[0xC] and event 1
    ///    (0x2586A0, bit 17) creates it for the controller on the cell of
    ///    step 3, as Monster Reborn's copy (the manual flip: 0x253D60 â†’
    ///    c6c low byte 2; in battle: 0x24AFA0 result << 16 â†’ c64 hi 2, the
    ///    battle resumes after the event). [F]
    pub(super) fn flip_graveyard_pick(
        &mut self,
        id: UnitId,
        low: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let unit = self.unit(id).expect("flipping unit").clone();
        let ctrl = unit.controller();
        if ctrl != self.active {
            return Ok(());
        }
        if self
            .far_trap_in(&self.foes_then_all(unit.side), 0x4C, cards)
            .is_some()
        {
            return Err(unsupported("trap effect type 0x4c"));
        }
        let Some(at) = self.leader_neighbour_cell(ctrl, true) else {
            return Ok(());
        };
        let (mode, filter) = (low >> 6, low & 0x3F);
        let graveyards = match mode {
            1 => vec![ctrl],
            3 => self.all_sides(),
            _ => {
                return Err(unsupported(format!(
                    "flip effect of card {} (type 0x14 mode {mode})",
                    unit.card
                )))
            }
        };
        if filter != 0x1E {
            return Ok(());
        }
        let count = graveyards
            .iter()
            .flat_map(|&g| self.sides[usize::from(g)].graveyard.iter())
            .filter(|&&c| cards.card(c).is_some_and(|c| !c.is_monster()))
            .count();
        if count == 0 {
            return Ok(());
        }
        if self.pending.is_some() {
            return Err(unsupported("a second selection opened in one action"));
        }
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: None,
        });
        self.pending = Some(Pending::GraveyardCopy {
            by: ctrl,
            at,
            graveyards,
            monsters: false,
            non_monsters: true,
        });
        events.push(DuelEvent::SelectionOpened { card: unit.card });
        Ok(())
    }

    /// Entry-1 type 2 on entry (0x251498, Dokurorider 128: word1 hi 0, lo
    /// 1): every cell within Manhattan distance hi of the entered cell
    /// (column outer, row inner) gets terrain lo through 0x1E34A0, a plain
    /// write: Labyrinth is not spared, unlike the flip and magic field
    /// changes. Record {0xFF, 0, g << 8 | s, 0}, result 1 (as Jirai Gumo's
    /// 0x42): the move goes on. Mode and filter are unread; no face check
    /// (0x252060). 0x247E10 ignores type 2: the unit walks normally. [F]
    pub(super) fn terrain_on_entry(
        &mut self,
        id: UnitId,
        cell: GridPos,
        word1: u32,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let (radius, value) = (word1 >> 16, (word1 & 0xFF) as u8);
        if value >= terrain::CRUSH {
            // A Crush cell under a unit is swept elsewhere; not traced here.
            return Err(unsupported(format!(
                "terrain {value} written on entry (0x251498)"
            )));
        }
        let unit = self.unit(id).expect("entering unit");
        let card = unit.card;
        if unit.position != cell {
            // Battle step 0xD without the advance (a Labyrinth cell).
            return Err(unsupported(
                "terrain on entry (0x251498) for a cell the unit did not enter",
            ));
        }
        events.push(DuelEvent::Activated {
            unit: id,
            card,
            target: None,
        });
        for (index, t) in self.terrain.iter_mut().enumerate() {
            let (c, r) = ((index % 7) as u8, (index / 7) as u8);
            if u32::from(c.abs_diff(cell.column) + r.abs_diff(cell.row)) <= radius {
                *t = value;
            }
        }
        Ok(())
    }

    /// Magic type 0x36 (builder 0x255CC8; 745 Call of the Haunted: mode 1,
    /// filter 0x1D, word1 2): mask = 0x24E360(0, 0, ctrl, mode, filter, 0xE)
    /// (slots 1..10 of both groups, controller by mode, the card filter),
    /// then 0x273C60(mask, ctrl); record {0x42, 0x36, mask, word1 & 0xFFFF},
    /// the card to the graveyard (0x250C90), an empty mask â†’ 0xFE (spent for
    /// nothing). Event 0x42 â†’ 0x26F220 (the flip retype's handler): the low
    /// byte of the value into each target's +0x3E (0x1E19E0) â€” 2 = Zombie.
    /// The builder has no type-0x37 arm. Called after the reveal/discard.
    /// [F; the handler as emu A of FLIP-112-POWERUPS-789-791.md]
    pub(super) fn magic_retype(
        &mut self,
        card: &DuelUnit,
        mode: u32,
        filter: u32,
        word1: u32,
        cards: &impl CardData,
    ) -> Result<()> {
        let ctrl = card.controller();
        let mut matched = Vec::new();
        for u in &self.units {
            if !(1..=10).contains(&u.slot) || u.id == card.id {
                continue;
            }
            let side_ok = match mode {
                1 => u.controller() == ctrl,
                2 => self.foe(u.controller(), ctrl),
                3 => true,
                _ => false,
            };
            if side_ok
                && select_filter(filter, &self.unit_card(u, cards))?
                && self.magic_target_admitted(u, ctrl, cards)
            {
                matched.push((u.side, u.slot, u.id));
            }
        }
        matched.sort_unstable();
        let value = (word1 & 0xFF) as u8;
        for (_, _, t) in matched {
            self.records.entry(t.0).or_default().kind = Some(value);
        }
        Ok(())
    }

    /// 0x24E360 criterion 0x20 (0x24F324, the jump table 0x2D3130 entry
    /// 0x20; Fissure 726): over the list 0x24E230(ctrl, mode) â€” group `ctrl`
    /// slots 1..10, then the other group's, each unit whose controller fits
    /// the mode (0x24F6A0) â€” the monsters (0x1E1980 class 0, any face) are
    /// compared by the word ATK << 16 | DEF at their own cell (0x252300 /
    /// 0x252460). The best starts at 0x2710_0000 (10000/0). A lower ATK
    /// wins. Otherwise the code compares the WHOLE best word with the
    /// candidate's ATK (0x24F3C0): only a best of ATK 0 whose DEF equals the
    /// candidate's ATK goes on to the DEF test (a lower DEF wins) and then
    /// the summon order test (0x1E2640: a lower order wins; an equal order
    /// wins only when the best's group equals `mode`, 0x24F3E8). Any other
    /// tie keeps the earlier unit. [F]
    pub(super) fn weakest_monster(
        &self,
        ctrl: u8,
        mode: u32,
        cards: &impl CardData,
    ) -> Option<UnitId> {
        let admitted = |u: &DuelUnit| match mode {
            1 => u.controller() == ctrl,
            2 => self.foe(u.controller(), ctrl),
            3 => true,
            _ => false,
        };
        let mut list: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| (1..=10).contains(&u.slot) && admitted(u))
            .collect();
        list.sort_by_key(|u| (u.side != ctrl, u.slot));
        let mut best: u32 = 0x2710_0000;
        let mut best_order: u32 = 0x270F;
        let mut best_group: i64 = -1;
        let mut pick = None;
        for u in list {
            if !self.unit_card(u, cards).is_monster() {
                continue;
            }
            let atk = u32::from(self.effective(u, u.position, true, cards));
            let def = u32::from(self.effective(u, u.position, false, cards));
            let order = u32::from(u.summon_order);
            let take = if atk < best >> 16 {
                true
            } else if best == atk {
                let best_def = best & 0xFFFF;
                if best_def != def {
                    def < best_def
                } else if order == best_order {
                    best_group == i64::from(mode)
                } else {
                    order < best_order
                }
            } else {
                false
            };
            if take {
                best = atk << 16 | def;
                best_order = order;
                best_group = i64::from(u.side);
                pick = Some(u.id);
            }
        }
        pick
    }
}
