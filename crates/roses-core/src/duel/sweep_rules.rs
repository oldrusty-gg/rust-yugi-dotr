//! Effects ported by the refusal sweep (PAL SLES_515.79; private notes
//! `claude-effects/REFUSALS-SWEEP.md`). Tags: [F] read from the code,
//! [emu] the game's own code in the EE interpreter (cases named in the
//! notes), [H] hypothesis.

use super::*;

impl Duel {
    /// Flip type 0x24 (0x266328): event 0x1E (0x25F790, The Wicked Worm
    /// Beast's return to the deck, [`Duel::return_to_deck`]) on
    /// - filter 0x1A (361 Wall of Illusion): the battle opponent, only in a
    ///   battle (role 1; role 0 has no opponent, role 2 is skipped):
    ///   result 0x201;
    /// - filter 0x1B (257 Hane-Hane): the unit itself; result 0x401 in a
    ///   battle, 1 otherwise;
    /// - any other filter: no record.
    ///
    /// [F; emu REFUSALS-SWEEP.md HH1 (manual: Hane-Hane appended to its
    /// deck), HH2 (attacked: Hane-Hane to its deck, the attacker advances,
    /// no damage), WI1 (the attacker to its deck, the Wall committed face-up
    /// defence), WI2 (the Wall attacks: the defender to its deck, the Wall
    /// advances), WI3 (manual: nothing)]; role 2 below.
    pub(super) fn flip_return_to_deck(
        &mut self,
        id: UnitId,
        role: FlipRole,
        low: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Flip> {
        let unit = self.unit(id).expect("flipping unit").clone();
        let target = match low & 0x3F {
            0x1A => match role {
                FlipRole::Battle(o) if self.unit(o).is_some_and(|u| u.slot != 0) => o,
                _ => return Ok(Flip::Continue),
            },
            0x1B => id,
            _ => return Ok(Flip::Continue),
        };
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: (target != id).then_some(target),
        });
        let t = self.unit(target).expect("target").clone();
        self.return_to_deck(&t, cards, events)?;
        // Role 2 (0x1B): result 1, so the combination step runs with the
        // mover's slot empty and its record 0x5F removes the partner
        // through 0x276F10 -> 0x250C90 [emu REFUSALS-SWEEP.md R13: Hane-Hane
        // back in the deck, the partner Red-Eyes in the graveyard].
        if let (Some(partner), true) = (role.partner(), target == id) {
            if self.unit(partner).is_some() {
                self.remove_by_effect(partner, cards, events)?;
            }
        }
        Ok(match role {
            FlipRole::Battle(_) if target == id => Flip::SelfDestroyed,
            FlipRole::Battle(_) => Flip::OpponentDestroyed,
            _ => Flip::Continue,
        })
    }
}

impl Duel {
    /// Event 0x2B (0x264500): a category-4 power-up that fits a monster
    /// (attacked by it, 0x2495D4, or moved onto it, 0x2493A0): record {0x2B,
    /// power-up, monster, transform(monster, power-up)}. The monster's old
    /// card goes to its owner's graveyard (0x1E00F0, no experience change)
    /// and the unit is rewritten in its slot (0x1E0D70: the new card, a new
    /// summon order, face-down attack position) on the target cell; the
    /// power-up goes through 0x250C90. [F; emu REFUSALS-SWEEP.md CE1 (an
    /// Acid Crawler attacking a set Cocoon of Evolution became Larva of Moth
    /// in its slot on the Cocoon's cell, acted), CE2 (a Cocoon moved onto an
    /// enemy Acid Crawler: the enemy's Larva, not acted), OC1/OC2 (own
    /// field: acted only when the monster was the mover)]. Insect Imitation 800
    /// draws its result ([`Duel::transform_drawn`]).
    pub(super) fn transform_by_power_up(
        &mut self,
        monster: UnitId,
        power: UnitId,
        at: GridPos,
        acted: bool,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let m = self.unit(monster).expect("monster").clone();
        let pcard = self.unit(power).expect("power-up").card;
        let mcard = self.unit_card(&m, cards).id;
        let Some(result) = self.transform_drawn(mcard, pcard) else {
            return Err(unsupported(format!("power-up {pcard} transforming card {mcard}")));
        };
        events.push(DuelEvent::Activated {
            unit: power,
            card: pcard,
            target: Some(monster),
        });
        let index = self.units.iter().position(|u| u.id == monster).expect("monster");
        let old = self.units.remove(index);
        self.sides[usize::from(old.side)].graveyard.push(old.card);
        events.push(DuelEvent::Destroyed {
            unit: old.id,
            card: old.card,
        });
        self.discard(power, events);
        if let Some(id) = self.create(old.side, result, at, cards, events) {
            if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                u.slot = old.slot;
                u.acted = acted;
            }
        }
        Ok(())
    }
}

impl Duel {
    /// 0x24A960 / its builder twin 0x24A540: [`transform`] plus table type 9
    /// (0x29C322[800], Insect Imitation), which draws one rand()
    /// (0x19AE88): Ryu-Ran 16 / Manga Ryu-Ran 17 → 13 + r % 5, Monster Egg
    /// 167 → 0xD1 + r % 15, Gorgon Egg 332 → 0x65 + (r & 0x1F), Wing Egg Elf 379 → one of the six Elves 0x23, 0x4F, 0x2C,
    /// 0x5C, 0x17B, 0x18A by r % 6 (0x24ADF0, table 0x2D3040). [F; emu
    /// REFUSALS-SWEEP.md II1: Manga Ryu-Ran became Yamadron 13 as the LCG
    /// predicted]
    pub(super) fn transform_drawn(&mut self, monster: u16, power_up: u16) -> Option<u16> {
        if power_up != 800 {
            return transform(monster, power_up);
        }
        let draw = match monster {
            16 | 17 | 167 | 332 | 379 => self.rand31(),
            _ => return None,
        };
        Some(match monster {
            16 | 17 => 13 + (draw % 5) as u16,
            167 => 0xD1 + (draw % 15) as u16,
            // 0x24ADF0: r % 6 over the jump table 0x2D3040.
            379 => [0x23, 0x4F, 0x2C, 0x5C, 0x17B, 0x18A][(draw % 6) as usize],
            _ => 0x65 + (draw & 0x1F) as u16,
        })
    }
}

/// An empty slot of the 40-slot deck array.
pub(super) const EMPTY_SLOT: u16 = 999;

impl Duel {
    /// Side +0x230: the deck count, stranded cards included (0x1DFC40).
    pub fn deck_count(&self, side: u8) -> usize {
        self.sides[usize::from(side)].deck.len() + self.deck_stranded[usize::from(side)].len()
    }

    /// 0x1DFC60(side, i): slot `i` of the 40-slot deck array: the
    /// compacted cards first, then 999s except where a stranded card sits.
    pub fn deck_slot(&self, side: u8, i: usize) -> u16 {
        let deck = &self.sides[usize::from(side)].deck;
        if let Some(&c) = deck.get(i) {
            return c;
        }
        self.deck_stranded[usize::from(side)]
            .iter()
            .find(|&&(slot, _)| usize::from(slot) == i)
            .map_or(EMPTY_SLOT, |&(_, c)| c)
    }

    /// 0x1DFCB0's write: `card` into the empty `slot`. A slot right after
    /// the compacted cards extends them (with any stranded card that then
    /// follows without a gap); a later slot strands the card.
    pub(super) fn place_in_deck(&mut self, side: u8, slot: u8, card: u16) {
        let g = usize::from(side);
        let stranded = &mut self.deck_stranded[g];
        stranded.push((slot, card));
        stranded.sort_unstable();
        loop {
            let next = self.sides[g].deck.len();
            let Some(i) = self.deck_stranded[g]
                .iter()
                .position(|&(slot, _)| usize::from(slot) == next)
            else {
                break;
            };
            let (_, c) = self.deck_stranded[g].remove(i);
            self.sides[g].deck.push(c);
        }
    }

    /// 0x1DFDA0(side): the deck array compacted, slot order.
    pub(super) fn compact_deck(&mut self, side: u8) {
        let g = usize::from(side);
        let stranded = std::mem::take(&mut self.deck_stranded[g]);
        self.sides[g].deck.extend(stranded.into_iter().map(|(_, c)| c));
    }

    /// 0x1DFFB8 on a deck whose slot 0 holds a card: it is taken, then
    /// 0x1DFDA0(side) compacts the drawing group's array.
    pub(super) fn draw_front(&mut self, side: u8) -> u16 {
        let card = self.sides[usize::from(side)].deck.remove(0);
        // The stranded slots shift with the removal before compaction; the
        // order is kept either way.
        self.compact_deck(side);
        card
    }
}

impl Duel {
    /// Turn start, entry-2 type 0x54 (0x268180; 676 Carat Idol: mode 3,
    /// filter 0x34 = any card): mask = 0x24E360(0, 0, ctrl, mode, filter,
    /// 0xE) minus itself, record {0x48, its cell, mask, ctrl}. Event 0x48
    /// (0x26E8F0 → 0x26ECF0 per round d = 0..=13): the mask units at
    /// Manhattan distance d from the Idol, taken by 0x250980 (lowest summon
    /// order; on a later lower-or-equal order only a unit of `ctrl` replaces
    /// the pick), each get a step toward the Idol (0x26EF30 → 0x26F010: the
    /// column step if that cell is empty, else the row step, else none; a
    /// step already chosen this round → the other choice, else none), and
    /// the round's steps are raw moves (0x1E0FD0) at its end. [F; emu
    /// REFUSALS-SWEEP.md CI1: seven units pulled one cell each, a Pact
    /// blocked by a cell chosen earlier in its round, Nekogal's column cell
    /// held by the leader took the row step; CI0 attack position: nothing]
    pub(super) fn carat_idol_pull(
        &mut self,
        idol: &DuelUnit,
        word0: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let (mode, filter) = ((word0 & 0xFF) >> 6, word0 & 0x3F);
        let ctrl = idol.controller();
        let mut mask: Vec<&DuelUnit> = Vec::new();
        for t in &self.units {
            let listed = t.slot != 0
                && t.id != idol.id
                && match mode {
                    1 => t.controller() == ctrl,
                    2 => self.foe(t.controller(), ctrl),
                    3 => true,
                    _ => false,
                };
            if listed && (matches!(filter, 0x34 | 0x1F) || select_filter(filter, &self.unit_card(t, cards))?) {
                mask.push(t);
            }
        }
        mask.sort_by_key(|t| (t.side, t.slot));
        let mask: Vec<UnitId> = mask.into_iter().map(|t| t.id).collect();
        if mask.is_empty() {
            return Ok(());
        }
        events.push(DuelEvent::Activated {
            unit: idol.id,
            card: idol.card,
            target: None,
        });
        let (tc, tr) = (idol.position.column, idol.position.row);
        let free = |d: &Duel, c: u8, r: u8| d.unit_at(GridPos { column: c, row: r }).is_none();
        let row_step = |d: &Duel, c: u8, r: u8| -> Option<GridPos> {
            let r2 = match r.cmp(&tr) {
                std::cmp::Ordering::Less => r + 1,
                std::cmp::Ordering::Greater => r - 1,
                std::cmp::Ordering::Equal => return None,
            };
            free(d, c, r2).then_some(GridPos { column: c, row: r2 })
        };
        let step = |d: &Duel, at: GridPos, other: bool| -> Option<GridPos> {
            let (c, r) = (at.column, at.row);
            let c2 = match c.cmp(&tc) {
                std::cmp::Ordering::Greater => c - 1,
                std::cmp::Ordering::Less => c + 1,
                std::cmp::Ordering::Equal => return row_step(d, c, r),
            };
            if free(d, c2, r) && !other {
                Some(GridPos { column: c2, row: r })
            } else {
                row_step(d, c, r)
            }
        };
        for dist in 0..=13u8 {
            let mut left: Vec<UnitId> = mask
                .iter()
                .copied()
                .filter(|&id| {
                    self.unit(id).is_some_and(|t| {
                        t.position.column.abs_diff(tc) + t.position.row.abs_diff(tr) == dist
                    })
                })
                .collect();
            let mut moves: Vec<(UnitId, GridPos)> = Vec::new();
            while !left.is_empty() {
                let mut best = 0xFFu8;
                let mut pick: Option<usize> = None;
                for (i, &id) in left.iter().enumerate() {
                    let t = self.unit(id).expect("listed");
                    if t.summon_order > best {
                        continue;
                    }
                    best = t.summon_order;
                    if pick.is_none() || t.controller() == ctrl {
                        pick = Some(i);
                    }
                }
                let id = left.remove(pick.expect("a pick"));
                let at = self.unit(id).expect("listed").position;
                let taken = |p: Option<GridPos>| p.is_some_and(|p| moves.iter().any(|&(_, q)| q == p));
                let mut dest = step(self, at, false);
                if taken(dest) {
                    dest = step(self, at, true);
                    if taken(dest) {
                        dest = None;
                    }
                }
                if let Some(p) = dest {
                    moves.push((id, p));
                }
            }
            for (id, p) in moves {
                self.advance(id, p, events);
            }
        }
        Ok(())
    }
}

impl Duel {
    /// Flip type 0x51 (0x266848; 672 Jowls of Dark Demise): nothing with a
    /// Celtic Guardian standing (0x268710(0x39)), without a non-leader
    /// battle opponent, or in role 2. Else record {0x43, self, opponent, 1}:
    /// the Jowls leaves the field without the graveyard, the opponent is
    /// revealed and taken by the controller of the Jowls (control bit set,
    /// acted kept), its record entry 2 becomes {0x51FD, 0} (the control
    /// survives turn starts, 0x2695EC) and entry 4 {0x515C, (ctrl << 8 |
    /// 0x1F) << 16 | 0x2A0} (back as a Jowls beside the leader when
    /// destroyed in battle, 0x253A6C; to its controller's graveyard at a battle
    /// removal that kept the entry, 0x24C544; 0x250C90 reads it after clearing
    /// the unit: nothing, emu JD5); result 0x101: the battle ends. [F 0x266848..0x266920; emu
    /// REFUSALS-SWEEP.md JD1 (Red-Eyes attacked a set Jowls: taken, f57
    /// 0x17, entries 2/4 rewritten, the Jowls gone), JD3, JD4]
    pub(super) fn flip_possess(
        &mut self,
        id: UnitId,
        role: FlipRole,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Flip> {
        let FlipRole::Battle(o) = role else {
            return Ok(Flip::Continue);
        };
        if self.defender_with(0x39, cards) || !self.unit(o).is_some_and(|u| u.slot != 0) {
            return Ok(Flip::Continue);
        }
        let jowls = self.unit(id).expect("flipping unit").clone();
        let ctrl = jowls.controller();
        events.push(DuelEvent::Activated {
            unit: id,
            card: jowls.card,
            target: Some(o),
        });
        if let Some(i) = self.units.iter().position(|u| u.id == id) {
            self.units.remove(i);
        }
        let mut entries = self.unit_card(self.unit(o).expect("opponent"), cards).entries;
        entries[2] = Some([0x51FD, 0]);
        entries[4] = Some([0x515C, (u32::from(ctrl) << 8 | 0x1F) << 16 | 0x2A0]);
        self.records.entry(o.0).or_default().entries = Some(entries);
        let multi = self.multi().is_some();
        if let Some(u) = self.units.iter_mut().find(|u| u.id == o) {
            u.face_up = true;
            u.committed_face_up = true;
            u.stolen = u.side != ctrl;
            // N-player duels (a port addition): who took it.
            u.stolen_by = (multi && u.stolen).then_some(ctrl);
        }
        Ok(Flip::BattleEnds)
    }

    /// Entry 4 type 0x51 (0x253A6C) of a possessed monster destroyed in
    /// battle: hi 0x1F: the first free neighbour of the controller's leader
    /// (0x24FB40(ctrl, 1)), 0x1E: of the other leader; none: nothing.
    /// Record {1, 0x10000 | ctrl, cell, word1}: event 1 creates card lo
    /// there for group word1 >> 24, revealed (bit 16), and the entry is
    /// erased (no second card at the removal). [F; emu REFUSALS-SWEEP.md
    /// JD3: Jowls face-up at (1,1) beside Basic Insect]
    pub(super) fn possessed_destroyed(
        &mut self,
        unit: &DuelUnit,
        word1: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        let ctrl = unit.controller();
        let side = match (word1 >> 16) & 0xFF {
            0x1F => ctrl,
            0x1E => self.opp(ctrl),
            _ => return,
        };
        let Some(cell) = self.leader_neighbour_cell(side, true) else {
            return;
        };
        if let Some(r) = self.records.get_mut(&unit.id.0) {
            if let Some(e) = r.entries.as_mut() {
                e[4] = None;
            }
        }
        events.push(DuelEvent::Activated {
            unit: unit.id,
            card: unit.card,
            target: None,
        });
        let group = ((word1 >> 24) & 1) as u8;
        if let Some(created) = self.create(group, (word1 & 0xFFFF) as u16, cell, cards, events) {
            if let Some(u) = self.units.iter_mut().find(|u| u.id == created) {
                u.face_up = true;
                u.committed_face_up = true;
            }
        }
    }
}
