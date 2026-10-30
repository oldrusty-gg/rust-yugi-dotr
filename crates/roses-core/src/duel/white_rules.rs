//! The card effects the white-route computer sessions met (Tea, Tristan,
//! Mako): Hourglass of Life 377 (entry-4 type 0x3E), Skelengel 384 (entry-4
//! type 0x26), the flips of Tongyo 470 (0x13), Change Slime 570 (0x16), The
//! Drdek 325 (0x23 with lo 0), Ooguchi 575 (0x25) and Mystery Hand 310
//! (0x3C), Creature Swap 751 (magic 0x25 / hi 0x21) and Binding Chain 386
//! (entry-2 type 0x17). Private notes:
//! `claude-effects/WHITE-REFUSALS.md`.

use super::*;

/// The role of a flip (0x265250's last argument): 0 a manual flip ending a
/// move, 1 a battle reveal (the opponent), 2 a move onto an own unit (the
/// partner).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FlipRole {
    Manual,
    Battle(UnitId),
    OntoOwn(UnitId),
}

impl FlipRole {
    pub(super) fn opponent(self) -> Option<UnitId> {
        match self {
            Self::Battle(o) => Some(o),
            _ => None,
        }
    }

    pub(super) fn partner(self) -> Option<UnitId> {
        match self {
            Self::OntoOwn(p) => Some(p),
            _ => None,
        }
    }
}

impl Duel {
    /// Entry-4 type 0x3E (0x253650; Hourglass of Life 377: mode 1, filter
    /// 0x1D, lo 1000), destroyed in battle by the enemy:
    /// 1. 0x26C4B0(g ^ 1, 0x4C): a type-0x4C trap (either group) answers
    ///    with event 0x66, or with the Decree record and result 3: refused,
    ///    as for Golgoil.
    /// 2. cost = 0x268A10(0x2C, lo) (the LP-change modifiers); nothing
    ///    unless cost < the physical owner's LP (0x1E06F0(g)).
    /// 3. With ctrl = the controller (0x24F790): nothing without a free
    ///    non-Labyrinth leader neighbour (0x24FB40(ctrl, 1)); count = the
    ///    least of 5 - the occupied slots 1..5 of group ctrl (0x2500A0(ctrl,
    ///    4), the dying unit still counted), the monsters (class 0) of both
    ///    graveyards (0x252260) and the free neighbours (0x24FC20(ctrl, n,
    ///    1), n < 8); 0 → nothing.
    /// 4. Event {0x3A, cost, ctrl, count} (0x26A3F0): the LP setter takes
    ///    the cost from ctrl (0x1E0710(ctrl, 1, cost)); then for n < count
    ///    the n-th free neighbour receives the strongest monster of the
    ///    graveyards (card ATK & 0x1FFF; ctrl's graveyard first, a later
    ///    entry winning a tie there, the other graveyard only when
    ///    stronger): 0x1E0800(ctrl, cell, card) = a fresh face-down unit in
    ///    attack position, then the acted bit, and the entry leaves its
    ///    graveyard (0x1E0280). [F; emu H3: Red-Eyes (enemy graveyard) and
    ///    Orion revived at (1,1) and (0,0) for 1000 LP; H4/H5]
    pub(super) fn hourglass_revive(
        &mut self,
        unit: &DuelUnit,
        word1: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Aftermath> {
        if self
            .far_trap_in(&self.foes_then_all(unit.side), 0x4C, cards)
            .is_some()
        {
            return Err(unsupported("trap effect type 0x4c"));
        }
        let cost = self.lp_modifier(0x2C, (word1 & 0xFFFF) as u16, cards);
        if cost >= self.sides[usize::from(unit.side)].lp {
            return Ok(Aftermath::Continue);
        }
        let ctrl = unit.controller();
        let cells = self.leader_neighbour_cells(ctrl, true);
        if cells.is_empty() {
            return Ok(Aftermath::Continue);
        }
        let occupied = self
            .units
            .iter()
            .filter(|u| u.side == ctrl && (1..=MONSTER_SLOTS as u8).contains(&u.slot))
            .count();
        let monsters = self
            .sides
            .iter()
            .flat_map(|s| s.graveyard.iter())
            .filter(|&&c| cards.card(c).is_some_and(CardInfo::is_monster))
            .count();
        let count = MONSTER_SLOTS
            .saturating_sub(occupied)
            .min(monsters)
            .min(cells.len());
        if count == 0 {
            return Ok(Aftermath::Continue);
        }
        events.push(DuelEvent::Activated {
            unit: unit.id,
            card: unit.card,
            target: None,
        });
        self.damage(ctrl, cost, events);
        for &cell in &cells[..count] {
            // 0x26A630..0x26A758: the strongest monster, own graveyard
            // first (>=: the later entry wins a tie), then the other one
            // (strictly stronger only).
            let mut best: Option<(u8, usize, u16)> = None;
            let groups: Vec<(u8, bool)> = std::iter::once((ctrl, false))
                .chain(self.foes(ctrl).into_iter().map(|g| (g, true)))
                .collect();
            for (g, strict) in groups {
                for (i, &c) in self.sides[usize::from(g)].graveyard.iter().enumerate() {
                    let Some(info) = cards.card(c).filter(|i| i.is_monster()) else {
                        continue;
                    };
                    let atk = info.attack & 0x1FFF;
                    let floor = best.map_or(0, |(_, _, a)| a);
                    if if strict { atk > floor } else { atk >= floor } {
                        best = Some((g, i, atk));
                    }
                }
            }
            let Some((g, i, _)) = best else {
                break;
            };
            let card = self.sides[usize::from(g)].graveyard[i];
            if let Some(id) = self.create(ctrl, card, cell, cards, &mut Vec::new()) {
                if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                    u.acted = true;
                }
                events.push(DuelEvent::Revived {
                    unit: id,
                    card,
                    at: cell,
                    from: g,
                });
            }
            self.sides[usize::from(g)].graveyard.remove(i);
        }
        Ok(Aftermath::Continue)
    }

    /// Entry-4 type 0x26 (0x253904; Skelengel 384, mode and filter unread):
    /// event {0x21, 0, ctrl, 0}, Solomon's Lawbook's handler 0x260760:
    /// 0x1DF980(ctrl, 1) sets the controller's summon flag (S+0x346 bit 0):
    /// one more summon this turn. [F; emu K1 record {0x21, 0, 1, 0}]
    pub(super) fn skelengel_summon(&mut self, unit: &DuelUnit, events: &mut Vec<DuelEvent>) {
        events.push(DuelEvent::Activated {
            unit: unit.id,
            card: unit.card,
            target: None,
        });
        self.sides[usize::from(unit.controller())].can_summon = true;
    }

    /// Flip type 0x13 (0x265508; Tongyo 470: mode 2, filter 0x1A, lo 500;
    /// mode and filter unread): role 1 against a non-leader opponent only
    /// (otherSlot > 0, otherG >= 0, role != 2): event {0x39, 0, opponent,
    /// lo}, the permanent -lo of F05 (0x2525C0 / 0x2527A0 op 1). [F; emu
    /// T1 Red-Eyes -500/-500 before the battle, T2 Hunter Spider -500 as
    /// the attacked unit, T3 a manual flip: nothing]
    pub(super) fn flip_lower_opponent(
        &mut self,
        id: UnitId,
        opponent: Option<UnitId>,
        lo: u16,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        let Some(o) = opponent else {
            return;
        };
        let card = self.unit(id).expect("flipping unit").card;
        events.push(DuelEvent::Activated {
            unit: id,
            card,
            target: Some(o),
        });
        self.lower_stats(o, i32::from(lo), cards, events);
    }

    /// Flip type 0x16 (0x265F24; Change Slime 570: mode 1, filter 0x1B, hi
    /// 0x1A): with hi 0x1A the battle opponent (role 1, non-leader), else
    /// the mask 0x24E360(0, 0, ctrl, 3, hi, 0xE) (no card: refused); event
    /// {0x40, mask, self, 0} (0x264AA0): per target the flipping unit's
    /// permanent ATK change gains the target's ATK at its own cell
    /// (0x252300) minus its own unclamped ATK at its own cell (0x2523E0)
    /// through 0x2525C0 op 0 (9999 cap), and the same for DEF (0x252460 /
    /// 0x252540 / 0x2527A0): it takes the opponent's stats. [F; emu C4:
    /// Slime 400/300 vs Red-Eyes 900/2500 → +500/+2200; C1/C2]
    pub(super) fn flip_copy_stats(
        &mut self,
        id: UnitId,
        opponent: Option<UnitId>,
        hi: u16,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let unit = self.unit(id).expect("flipping unit").clone();
        if hi != 0x1A {
            return Err(unsupported(format!(
                "flip effect of card {} (type 0x16 hi {hi:#x})",
                unit.card
            )));
        }
        let Some(o) = opponent else {
            return Ok(());
        };
        let target = self.unit(o).expect("opponent").clone();
        let (atk, def) = (
            i32::from(self.effective(&target, target.position, true, cards)),
            i32::from(self.effective(&target, target.position, false, cards)),
        );
        let (own_atk, own_def) = (
            self.effective_sum(&unit, unit.position, true, cards),
            self.effective_sum(&unit, unit.position, false, cards),
        );
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: Some(o),
        });
        self.add_stat_capped(id, true, atk - own_atk, cards);
        self.add_stat_capped(id, false, def - own_def, cards);
        if let Some(u) = self.unit(id) {
            events.push(DuelEvent::Modified {
                unit: id,
                attack: u.attack_bonus,
                defense: u.defense_bonus,
            });
        }
        Ok(())
    }

    /// 0x2525C0 / 0x2527A0 op 0 on one stat: when clamp(base + change) +
    /// value reaches 10000 the change becomes 9999 - base, else it gains
    /// the value (no floor).
    fn add_stat_capped(&mut self, id: UnitId, attack: bool, value: i32, cards: &impl CardData) {
        let Some(u) = self.units.iter_mut().find(|u| u.id == id) else {
            return;
        };
        let card = cards.card(u.card).expect("validated card");
        let (base, delta) = if attack {
            (i32::from(card.attack), &mut u.attack_bonus)
        } else {
            (i32::from(card.defense), &mut u.defense_bonus)
        };
        let current = (base + i32::from(*delta)).clamp(0, i32::from(MAX_LP));
        let next = if current + value >= 10000 {
            i32::from(MAX_LP) - base
        } else {
            i32::from(*delta) + value
        };
        *delta = next.clamp(-i32::from(MAX_LP), i32::from(MAX_LP)) as i16;
    }

    /// Flip type 0x23 (0x265AE4; Key Mace 374 mode 3 filter 0x34 lo 1, The
    /// Drdek 325 mode 3 filter 0x1F lo 0): mask = 0x24E360(0, 0, ctrl, mode,
    /// filter, 0xE) minus itself (0x24E0F0) and, in role 2, minus the
    /// partner; event {0xD, 5, mask, lo}: 0x25B980 mode 5 writes +0x57 bits
    /// 5 and 3 := lo (0x1E30F0) on every target: lo 1 committed defence,
    /// lo 0 committed attack; face and acted untouched. [F; emu D1: own
    /// Red-Eyes 0x3C → 0x14, enemy face-down 0x29 → 0x01 (set cards too);
    /// D2 as a defender the attacker is in the mask]
    pub(super) fn flip_positions(
        &mut self,
        id: UnitId,
        role: FlipRole,
        low: u32,
        lo: u16,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let unit = self.unit(id).expect("flipping unit").clone();
        let (mode, filter) = (low >> 6, low & 0x3F);
        if filter != 0x34 {
            select_filter(filter, &self.unit_card(&unit, cards))?;
        }
        let ctrl = unit.controller();
        let mut targets: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && u.id != id
                    && Some(u.id) != role.partner()
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        3 => true,
                        _ => false,
                    }
                    && (filter == 0x34
                        || select_filter(filter, &self.unit_card(u, cards)).unwrap_or(false))
            })
            .collect();
        targets.sort_by_key(|u| (u.side, u.slot));
        let targets: Vec<UnitId> = targets.into_iter().map(|u| u.id).collect();
        if targets.is_empty() {
            return Ok(());
        }
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: None,
        });
        let defence = lo != 0;
        for t in targets {
            if let Some(u) = self.units.iter_mut().find(|u| u.id == t) {
                u.defence = defence;
                u.committed_defence = defence;
            }
        }
        Ok(())
    }

    /// Flip type 0x25 (0x266610; Ooguchi 575: mode 1, filter 0x1B, hi
    /// 0x1E): hi 0x1E → the first free non-Labyrinth neighbour of the
    /// OTHER side's leader (0x24FB40(ctrl ^ 1, 1)), hi 0x1F → of its own
    /// leader's, any other hi (0x20 included) → nothing; no cell →
    /// nothing. Event {0x1F, self, 0, cell} = Shift's raw move 0x1E0FD0
    /// (flags untouched). Result 0x801 in role 1: the battle ends (step
    /// 0xE: no damage, no advance). [F; emu O1 the defender went to (4,6)
    /// beside the enemy leader, the attacker stayed; O2 the attacker went
    /// to (1,1), the defender untouched; O3 a manual flip]
    pub(super) fn flip_teleport(
        &mut self,
        id: UnitId,
        role: FlipRole,
        hi: u16,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Flip> {
        let unit = self.unit(id).expect("flipping unit").clone();
        let ctrl = unit.controller();
        let side = match hi {
            0x1E => self.opp(ctrl),
            0x1F => ctrl,
            _ => return Ok(Flip::Continue),
        };
        let Some(cell) = self.leader_neighbour_cell(side, true) else {
            return Ok(Flip::Continue);
        };
        // Role 2: the raw move, result 1, then the combination (step 5)
        // as if the mover had not left: it ends on the partner's cell [emu
        // REFUSALS-SWEEP.md R11 Turtle Raccoon / R12 Ooguchi: event 0x1F
        // moved it beside a leader, the failed fusion then left it on the
        // partner's cell with the partner in the graveyard].
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: None,
        });
        self.advance(id, cell, events);
        Ok(if role.opponent().is_some() {
            Flip::BattleEnds
        } else {
            Flip::Continue
        })
    }

    /// Flip type 0x3C (0x2666FC; Mystery Hand 310: e3 0x3C5C, word1
    /// 0x1E026D; mode and hi unread): mask = 0x24E360(col, row, ctrl, 3,
    /// 0x1C | lo << 16, 1) = units in slots 1..10 of both groups within
    /// Manhattan distance 1 of the flipping unit's cell whose card id is lo
    /// (0x26D Jigen Bakudan), any face; empty → nothing; then the first
    /// free non-Labyrinth neighbour of the other side's leader
    /// (0x24FB40(ctrl ^ 1, 1)); none → nothing; the target is the mask's
    /// lowest summon order (0x2508D0, the first in mask order on a tie);
    /// event {0x1F, target, 0, cell}: a raw move (Shift's handler). Role 2
    /// with the partner moved returns 0x201 (step 0x20B: not traced). [F;
    /// emu M1 own Jigen, M2 enemy Jigen moved to (1,1); M3 distance 2:
    /// nothing; M4 the enemy Jigen (order 11) before the own (12)]
    pub(super) fn flip_send_neighbour(
        &mut self,
        id: UnitId,
        role: FlipRole,
        lo: u16,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let unit = self.unit(id).expect("flipping unit").clone();
        let at = unit.position;
        let mut pick: Option<(u8, (u8, u8), UnitId)> = None;
        for u in &self.units {
            if u.slot == 0
                || u.card != lo
                || u32::from(
                    u.position.column.abs_diff(at.column) + u.position.row.abs_diff(at.row),
                ) > 1
            {
                continue;
            }
            let key = (u.summon_order, (u.side, u.slot));
            if pick.is_none_or(|(o, m, _)| (key.0, key.1) < (o, m)) {
                pick = Some((key.0, key.1, u.id));
            }
        }
        let Some((_, _, target)) = pick else {
            return Ok(());
        };
        let Some(cell) = self.leader_neighbour_cell(self.opp(unit.controller()), true) else {
            return Ok(());
        };
        if role.partner() == Some(target) {
            return Err(unsupported(format!(
                "flip effect of card {} moving its partner away",
                unit.card
            )));
        }
        if role.opponent() == Some(target) {
            // Result 1: the battle would go on with the opponent elsewhere.
            return Err(unsupported(format!(
                "flip effect of card {} moving the battle opponent",
                unit.card
            )));
        }
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: Some(target),
        });
        self.advance(target, cell, events);
        Ok(())
    }

    /// Magic type 0x25 with hi 0x21 (0x255E28; Creature Swap 751: mode 3,
    /// filter 0x34): mask = 0x24E360(0, 0, ctrl, mode, filter, 0xE) minus
    /// the card and both leaders, then the immunity filter 0x273C60(mask,
    /// ctrl); event {0x20, 0, mask, 0} after the card left (0x250C90);
    /// empty → 0xFE (nothing). Handler 0x260030, per listed unit in mask
    /// order (group 0 slots 1..10, then group 1), skipping units already
    /// moved: its cell (c, r) is mirrored to (r, c) (0x1E4A10(r, c)); an
    /// empty cell takes it (0x1E0FD0, raw); a leader there keeps it in
    /// place; another unit there swaps cells with it (0x260500, through
    /// the first empty cell) and leaves the list, unless 0x2738C0
    /// (leader ability #11, refused at setup) protects it. [F; emu S1:
    /// every unit mirrored, (2,4) ↔ (4,2), (3,2) ↔ (2,3) swapped]
    pub(super) fn creature_swap(
        &mut self,
        unit: &DuelUnit,
        mode: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        let ctrl = unit.controller();
        let mut listed: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && u.id != unit.id
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        _ => true,
                    }
                    && self.magic_target_admitted(u, ctrl, cards)
            })
            .collect();
        listed.sort_by_key(|u| (u.side, u.slot));
        let mut listed: Vec<Option<UnitId>> = listed.into_iter().map(|u| Some(u.id)).collect();
        for k in 0..listed.len() {
            let Some(id) = listed[k] else {
                continue;
            };
            let Some(from) = self.unit(id).map(|u| u.position) else {
                continue;
            };
            let to = GridPos {
                column: from.row,
                row: from.column,
            };
            match self.unit_at(to).map(|o| (o.id, o.slot)) {
                None => self.advance(id, to, events),
                Some((_, 0)) => (),
                // 0x2602A4: an occupant under leader ability #11 at the
                // mover's cell (its destination, 0x2738C0) stays; so does
                // the mover.
                Some((other, _))
                    if self
                        .unit(other)
                        .is_some_and(|o| self.ability11_at(o, from, cards)) => {}
                Some((other, _)) => {
                    if other != id {
                        self.advance(other, from, events);
                        self.advance(id, to, events);
                    }
                    for entry in listed.iter_mut() {
                        if *entry == Some(other) {
                            *entry = None;
                        }
                    }
                }
            }
        }
    }
}

impl Duel {
    /// Unit +0x4A (0x1E28C0) is set.
    pub(super) fn is_chain_bound(&self, id: UnitId) -> bool {
        self.chain_bound.get(&id.0).is_some_and(|&b| b != 0)
    }

    /// The type-0x17 entry-2 mask of `source` (0x24E360(0, 0, ctrl, mode,
    /// filter, 0xE)): units in slots 1..10 of both groups whose controller
    /// fits the mode relative to the source's controller (0x24F790) and
    /// whose card passes the filter, any face, the source included.
    fn chain_mask(&self, source: &DuelUnit, word0: u32, cards: &impl CardData) -> Vec<UnitId> {
        let (mode, filter) = ((word0 & 0xFF) >> 6, word0 & 0x3F);
        let ctrl = source.controller();
        let mut v: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        3 => true,
                        _ => false,
                    }
                    && select_filter(filter, &self.unit_card(u, cards)).unwrap_or(false)
            })
            .collect();
        v.sort_by_key(|u| (u.side, u.slot));
        v.into_iter().map(|u| u.id).collect()
    }

    /// The entry-2 type-0x17 words of a unit, when it has them (its record's
    /// entries, 0x252060(g, s, 2)).
    fn chain_words(&self, u: &DuelUnit, cards: &impl CardData) -> Option<u32> {
        self.unit_card(u, cards).entries[2]
            .map(|[w, _]| w)
            .filter(|w| w >> 8 == 0x17)
    }

    /// 0x2683A0(g, s): some unit (group 0 slots 0..10, then group 1) with
    /// an entry-2 type 0x17 in committed face-up defence (+0x57 bits 4, 3
    /// and 5) has `target` in its mask.
    fn chain_covered(&self, target: UnitId, cards: &impl CardData) -> bool {
        self.units.iter().any(|u| {
            u.committed_face_up
                && u.defence
                && u.committed_defence
                && self
                    .chain_words(u, cards)
                    .is_some_and(|w| self.chain_mask(u, w, cards).contains(&target))
        })
    }

    /// Turn start 0x269570 phases 0x26..0x2A (0x269808): for each slot 1..5
    /// of the group that is NOT active, a unit in committed face-up and
    /// committed defence (+0x57 bits 4 and 5) whose control did not change
    /// (bit 1 clear; every control change has ended by now) runs 0x268230:
    /// entry-2 type 0x17 → its mask minus the units whose +0x4A is set
    /// (0x1E28C0); empty → nothing; else event {0, 1, mask, lo}: the event-0
    /// handler 0x2579E0 sees rec[4] = 1 and writes +0x4A := 0xFF on every
    /// target (0x1E2860), nothing else (no +0x49, no acted bit). Phase 0x40
    /// then marks the active group's +0x4A units acted. [F]
    pub(super) fn chain_bind_turn_start(
        &mut self,
        s: u8,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        // N-player duels (a port addition): every enemy group, in turn order.
        for (g, slot) in self
            .enemy_groups(s)
            .into_iter()
            .flat_map(|g| (1..=MONSTER_SLOTS as u8).map(move |slot| (g, slot)))
        {
            let Some(u) = self
                .units
                .iter()
                .find(|u| u.side == g && u.slot == slot)
                .cloned()
            else {
                continue;
            };
            if !(u.committed_face_up && u.committed_defence) || u.stolen {
                continue;
            }
            let Some(word0) = self.chain_words(&u, cards) else {
                continue;
            };
            let targets: Vec<UnitId> = self
                .chain_mask(&u, word0, cards)
                .into_iter()
                .filter(|&t| !self.is_chain_bound(t))
                .collect();
            if targets.is_empty() {
                continue;
            }
            events.push(DuelEvent::Activated {
                unit: u.id,
                card: u.card,
                target: None,
            });
            for t in targets {
                self.chain_bound.insert(t.0, 0xFF);
            }
        }
    }

    /// 0x273E20 (0x274370 / 0x274440): at every turn end, each unit of
    /// either group whose +0x4A is set and that no type-0x17 card covers
    /// any more (0x2683A0) gets +0x4A := 0 (0x1E2860). The leader-ability
    /// #15 branch before it is refused at setup.
    pub(super) fn chain_bind_turn_end(&mut self, cards: &impl CardData) {
        let live: Vec<UnitId> = self
            .units
            .iter()
            .filter(|u| self.is_chain_bound(u.id))
            .map(|u| u.id)
            .collect();
        let keep: Vec<UnitId> = live
            .into_iter()
            .filter(|&id| self.chain_covered(id, cards) || self.ability15_keeps(id, cards))
            .collect();
        self.chain_bound.retain(|id, _| keep.contains(&UnitId(*id)));
    }
}

impl Duel {
    /// Magic type 4 with hi 0x13 (0x254F20 → 0x255070; Fairy Meteor Crush
    /// 731, `0x04DF, 0x130000`): the builder writes record **0x19** (v0 is
    /// still 0x19 from the delay slot of 0x254F28), rec[8] = 0x2501A0 over
    /// the untouched cell list (garbage, unread) and rec[0xC] = the
    /// activator; the card leaves first (0x250C90). Handler 0x25DC50:
    /// 1. Init: ten cells, each two 0x19AE88 draws, row = first % 7, column
    ///    = second % 7 (stored row << 8 | column).
    /// 2. Every 8 updates the next strike launches (sub-script 0x20D on its
    ///    cell; earlier strikes on the same cell are marked done). At each
    ///    strike's impact the unit on its cell (0x1E4A10), if any and not a
    ///    leader, is hit (sub-script 0x25B) unless leader ability #11
    ///    (refused at setup) resists or the Dragon immunity (0x269020)
    ///    protects a unit not controlled by the activator (message 0x53);
    ///    hit units go to their owners' graveyards (0x250C90, no entry 4)
    ///    when both sub-scripts end. Either side's units can be hit.
    /// [F; emu F1: cells 0x606, 0x206, 0x505 ×3, 0x301, 0x104, 0x603, 0x404,
    /// 0x101 from LCG state 0x289E0CD0081492B6 exactly as predicted; Big
    /// Insect (1,3) then the Pact (4,1) destroyed, the leader at (3,6)
    /// untouched; F2 (state 0x123456789ABCDEF0): Hunter Spider (2,4) on the
    /// last strike.] A unit is removed at the first strike on its cell [H:
    /// the graveyard order of repeated cells depends on module timing].
    pub(super) fn meteor_strikes(
        &mut self,
        unit: &DuelUnit,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let mut cells = Vec::with_capacity(10);
        for _ in 0..10 {
            let row = (self.rand31() % 7) as u8;
            let column = (self.rand31() % 7) as u8;
            cells.push(GridPos { column, row });
        }
        let by = unit.controller();
        for cell in cells {
            let Some(target) = self.unit_at(cell).filter(|u| u.slot != 0).cloned() else {
                continue;
            };
            // 0x25E030: leader ability #11 resists whoever controls the
            // unit; then the 0x269020 immunity for the other side only.
            if self.ability11(&target, cards) {
                continue;
            }
            if !self.magic_target_admitted(&target, by, cards) {
                continue;
            }
            self.remove_by_effect(target.id, cards, events)?;
        }
        Ok(())
    }
}

impl Duel {
    /// Magic type 0x22 (0x255B7C; Yellow Luster Shield 737, `0x225D, 0x384`):
    /// mask = 0x24E360(0, 0, ctrl, mode, filter, 0xE) minus the card, then
    /// 0x273C60(mask, ctrl); record {0x1D, 0, mask, lo} (empty: 0xFE). Event
    /// 0x1D (0x25EF60) always runs 0x2527A0(op rec[0xC] >> 16 = 0, lo) on
    /// DEF and adds 0x2525C0 on ATK only when rec[4] == 1: DEF +lo, capped
    /// at 9999. [F; emu Y1: own Nekogal 2000 -> 2900 and face-down Red-Eyes
    /// 2500 -> 3400 DEF, ATK unchanged; record {0x1D, 0, 0x180000, 0x384}]
    pub(super) fn magic_defence_boost(
        &mut self,
        unit: &DuelUnit,
        mode: u32,
        filter: u32,
        lo: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        let ctrl = unit.controller();
        let mut targets: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && u.id != unit.id
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        _ => true,
                    }
                    && select_filter(filter, &self.unit_card(u, cards)).unwrap_or(false)
                    && self.magic_target_admitted(u, ctrl, cards)
            })
            .collect();
        targets.sort_by_key(|u| (u.side, u.slot));
        let targets: Vec<UnitId> = targets.into_iter().map(|u| u.id).collect();
        for t in targets {
            self.add_stat_capped(t, false, lo as i32, cards);
            if let Some(u) = self.unit(t) {
                events.push(DuelEvent::Modified {
                    unit: t,
                    attack: u.attack_bonus,
                    defense: u.defense_bonus,
                });
            }
        }
    }
}

/// 0x273A20(side, kind): the kinds leader ability #16 destroys, by the
/// leader's kind & 0x1F.
fn ability16_kind(leader: u8, kind: u8) -> bool {
    match leader & 0x1F {
        0x13 => kind == 0x12,
        0x12 => kind == 0xA,
        0x11 => matches!(kind, 2 | 0x13),
        0xB => kind == 9,
        8 => kind == 7,
        _ => false,
    }
}

/// 0x273980(side, kind): the kinds leader ability #15 binds.
fn ability15_kind(leader: u8, kind: u8) -> bool {
    match leader & 0x1F {
        0x13 => kind == 0,
        0x11 => matches!(kind, 5 | 6),
        0xB => kind == 0x10,
        8 => kind == 2,
        _ => false,
    }
}

/// Whether the action ends after a #13 reveal (0x24CC30 result 0x10201):
/// it does not [emu W4: a face-down attacker revealed at the target cell
/// still fights and advances].
const WINDOW_REVEAL_STOPS: bool = false;

impl Duel {
    fn leader_kind(&self, side: u8, cards: &impl CardData) -> u8 {
        self.unit_card(self.leader(side), cards).kind & 0x1F
    }

    /// 0x273DF0 + 0x273280: `cell` lies in the square of half-size 1 (2
    /// with ability #9) around `center`, clipped to the board.
    fn in_window_at(
        &self,
        side: u8,
        center: GridPos,
        cell: GridPos,
        cards: &impl CardData,
    ) -> bool {
        let half = if self.leader_has(side, 9, cards) {
            2
        } else {
            1
        };
        center.column.abs_diff(cell.column) <= half && center.row.abs_diff(cell.row) <= half
    }

    /// Event 0x71 (0x2751E0), after its presentation: every unit of the
    /// #16 mask goes to its owner's graveyard (0x250C90, no entry 4), every
    /// unit of the #15 mask gets +0x4A := 0xFF and the acted bit, every
    /// unit of the #13 mask is revealed (0x252FF0(g, s, 1)). Masks in mask
    /// order (group 0, then 1, by slot).
    fn ability_event_71(
        &mut self,
        mut destroy: Vec<UnitId>,
        mut bind: Vec<UnitId>,
        mut reveal: Vec<UnitId>,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let key = |d: &Duel, id: &UnitId| d.unit(*id).map(|u| (u.side, u.slot));
        destroy.sort_by_key(|id| key(self, id));
        bind.sort_by_key(|id| key(self, id));
        reveal.sort_by_key(|id| key(self, id));
        for id in destroy {
            if self.unit(id).is_some() {
                self.remove_by_effect(id, cards, events)?;
            }
        }
        for id in bind {
            if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                u.acted = true;
                self.chain_bound.insert(id.0, 0xFF);
            }
        }
        for id in reveal {
            if self.unit(id).is_some() {
                self.expose(id, events);
            }
        }
        Ok(())
    }

    /// 0x24CC30 with t1 = 0 (walker state 0x36 and the attack target check),
    /// before the trap search: when `cell` lies in the window of the leader
    /// L of the mover's controller's opponent (0x273370):
    /// - a monster (record class 0): #16 of L with 0x273A20(L, kind) puts
    ///   it in the first mask, #15 with 0x273980(L, kind) in the second
    ///   (no +0x4A test here); either: {0x71, m16, m15, 0x80000000},
    ///   result 0x101 (the walk ends);
    /// - otherwise (and for non-monsters): #13 of L, the unit neither shown
    ///   (+0x57 bit 2) nor committed face-up (bit 4): {0x71, 0, 0,
    ///   0x80000000 | m13}, result 0x10201.
    ///
    /// Returns Some(stop) when an event ran. [F 0x24CC9C..0x24CE30; emu]
    pub(super) fn window_entry_abilities(
        &mut self,
        mover: UnitId,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Option<bool>> {
        let unit = self.unit(mover).expect("mover").clone();
        // N-player duels (a port addition): each enemy leader's window the
        // mover enters, in turn order from the mover's controller.
        for l in self.enemy_leaders(unit.controller()) {
            if let Some(stop) = self.window_entry_ability(mover, &unit, l, cell, cards, events)? {
                return Ok(Some(stop));
            }
        }
        Ok(None)
    }

    /// [`Self::window_entry_abilities`] for the window of `l`'s leader.
    fn window_entry_ability(
        &mut self,
        mover: UnitId,
        unit: &DuelUnit,
        l: u8,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Option<bool>> {
        if !self.in_window(l, cell, cards) {
            return Ok(None);
        }
        let card = self.unit_card(unit, cards).into_owned();
        let lk = self.leader_kind(l, cards);
        if card.is_monster() {
            let kind = card.kind & 0x1F;
            let destroy = self.leader_has(l, 16, cards) && ability16_kind(lk, kind);
            let bind = self.leader_has(l, 15, cards) && ability15_kind(lk, kind);
            if destroy || bind {
                events.push(DuelEvent::Activated {
                    unit: self.leader(l).id,
                    card: self.leader(l).card,
                    target: Some(mover),
                });
                self.ability_event_71(
                    if destroy { vec![mover] } else { vec![] },
                    if bind { vec![mover] } else { vec![] },
                    vec![],
                    cards,
                    events,
                )?;
                return Ok(Some(true));
            }
        }
        if self.leader_has(l, 13, cards) && !unit.face_up && !unit.committed_face_up {
            events.push(DuelEvent::Activated {
                unit: self.leader(l).id,
                card: self.leader(l).card,
                target: Some(mover),
            });
            self.ability_event_71(vec![], vec![], vec![mover], cards, events)?;
            return Ok(Some(WINDOW_REVEAL_STOPS));
        }
        Ok(None)
    }

    /// 0x250E40 (after its Crush test) for a summoned unit: the same as
    /// [`Duel::window_entry_abilities`] at the unit's cell, except that #13
    /// tests only +0x57 bit 4. [F]
    pub(super) fn window_summon_abilities(
        &mut self,
        id: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let Some(unit) = self.unit(id).cloned() else {
            return Ok(());
        };
        // N-player duels (a port addition): the first enemy leader whose
        // window holds the unit, in turn order.
        let Some(l) = self
            .enemy_leaders(unit.controller())
            .into_iter()
            .find(|&l| self.in_window(l, unit.position, cards))
        else {
            return Ok(());
        };
        if unit.slot == 0 {
            return Ok(());
        }
        let card = self.unit_card(&unit, cards).into_owned();
        let lk = self.leader_kind(l, cards);
        let kind = card.kind & 0x1F;
        let monster = card.is_monster();
        let destroy = monster && self.leader_has(l, 16, cards) && ability16_kind(lk, kind);
        let bind = monster && self.leader_has(l, 15, cards) && ability15_kind(lk, kind);
        if destroy || bind {
            self.ability_event_71(
                if destroy { vec![id] } else { vec![] },
                if bind { vec![id] } else { vec![] },
                vec![],
                cards,
                events,
            )?;
        } else if self.leader_has(l, 13, cards) && !unit.committed_face_up {
            self.ability_event_71(vec![], vec![], vec![id], cards, events)?;
        }
        Ok(())
    }

    /// The window sweep of side `l`'s leader around `center` (0x273E20 phase
    /// 2 at the end of `l`'s turn, around the leader's cell; 0x251150 when
    /// the leader enters `center`): every non-leader unit controlled by the
    /// other side inside the window (0x273DF0 size): monsters with 0x273A20
    /// → #16 mask; monsters with 0x273980 whose +0x4A is clear → #15 mask;
    /// any unit not committed face-up (+0x57 bit 4) → #13 mask, each gated
    /// by the ability; any mask → event 0x71. [F 0x273EFC..0x274104,
    /// 0x2511D0..0x251374]
    pub(super) fn window_sweep(
        &mut self,
        l: u8,
        center: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let (a16, a15, a13) = (
            self.leader_has(l, 16, cards),
            self.leader_has(l, 15, cards),
            self.leader_has(l, 13, cards),
        );
        if !(a16 || a15 || a13) {
            return Ok(());
        }
        let lk = self.leader_kind(l, cards);
        let (mut destroy, mut bind, mut reveal) = (vec![], vec![], vec![]);
        for u in &self.units {
            if u.slot == 0
                || !self.foe(u.controller(), l)
                || !self.in_window_at(l, center, u.position, cards)
            {
                continue;
            }
            let card = self.unit_card(u, cards);
            let kind = card.kind & 0x1F;
            if card.is_monster() {
                if a16 && ability16_kind(lk, kind) {
                    destroy.push(u.id);
                }
                if a15 && ability15_kind(lk, kind) && !self.is_chain_bound(u.id) {
                    bind.push(u.id);
                }
            }
            if a13 && !u.committed_face_up {
                reveal.push(u.id);
            }
        }
        if destroy.is_empty() && bind.is_empty() && reveal.is_empty() {
            return Ok(());
        }
        events.push(DuelEvent::Activated {
            unit: self.leader(l).id,
            card: self.leader(l).card,
            target: None,
        });
        self.ability_event_71(destroy, bind, reveal, cards, events)
    }

    /// 0x274310 / 0x2743D8: at a turn end a unit of group G keeps +0x4A
    /// while the leader of group G ^ 1 has #15, 0x273980 matches the unit's
    /// kind and the unit stands in that leader's window.
    fn ability15_keeps(&self, id: UnitId, cards: &impl CardData) -> bool {
        let Some(u) = self.unit(id) else {
            return false;
        };
        self.enemy_leaders(u.side).into_iter().any(|l| {
            self.leader_has(l, 15, cards)
                && ability15_kind(
                    self.leader_kind(l, cards),
                    self.unit_card(u, cards).kind & 0x1F,
                )
                && self.in_window_at(l, self.leader(l).position, u.position, cards)
        })
    }
}

impl Duel {
    /// Flip type 0x1F (0x266464; 545 Mega Thunderball: mode 2, filter 0x1A,
    /// hi 0xE):
    /// 1. A defending Celtic Guardian anywhere (0x268710(0x39) > 0):
    ///    nothing.
    /// 2. Filter 0x1A: the battle opponent (role 1, non-leader); with hi 0xE
    ///    it must be a Machine, read as 0x1E1980(opponent's CONTROLLER,
    ///    opponent's slot) (the group argument is the controller: a stolen
    ///    opponent reads the other group's record in that slot); else
    ///    nothing. Other filters (a 0x24E360 mask): refused (no card).
    /// 3. Event {0x1A, ctrl, mask, 0xFFFE}: Brain Control's automatic path
    ///    (0x25E564): per target, leader ability #11 (0x273770) or the
    ///    0x269020 immunity → nothing, the same controller → nothing, else
    ///    control changes. Result 0x101 in role 1: the battle ends (step
    ///    0xE), 1 otherwise. [F]
    pub(super) fn flip_take_control(
        &mut self,
        id: UnitId,
        role: FlipRole,
        low: u32,
        hi: u16,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Flip> {
        let unit = self.unit(id).expect("flipping unit").clone();
        if self.defender_with(0x39, cards) {
            return Ok(Flip::Continue);
        }
        if low & 0x3F != 0x1A {
            // 0x266578 (677 Electromagnetic Bagworm: mode 2, filter 0xE):
            // mask = 0x24E360(0, 0, ctrl, mode, filter, 0xE), record {0x1A,
            // ctrl, mask, 0xFFFE}: Brain Control's automatic path over the
            // whole mask; result 0x101 in a battle when the opponent is in
            // it (the battle ends), else 1 [F; emu REFUSALS-SWEEP.md BW1:
            // both enemy Machines, any face, taken; BW2: a Machine attacker
            // taken, no battle].
            let (mode, filter) = (low >> 6, low & 0x3F);
            let ctrl = unit.controller();
            let mut mask: Vec<&DuelUnit> = Vec::new();
            for u in &self.units {
                let listed = u.slot != 0
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        3 => true,
                        _ => false,
                    };
                if listed && select_filter(filter, &self.unit_card(u, cards))? {
                    mask.push(u);
                }
            }
            mask.sort_by_key(|u| (u.side, u.slot));
            let mask: Vec<DuelUnit> = mask.into_iter().cloned().collect();
            if mask.is_empty() {
                return Ok(Flip::Continue);
            }
            events.push(DuelEvent::Activated {
                unit: id,
                card: unit.card,
                target: None,
            });
            for o in &mask {
                if !(self.ability11(o, cards) || o.controller() == ctrl || self.dragon_immunity(o, cards)) {
                    self.change_control(o.id, ctrl, events);
                }
            }
            return Ok(match role.opponent() {
                Some(o) if mask.iter().any(|u| u.id == o) => Flip::BattleEnds,
                _ => Flip::Continue,
            });
        }
        let Some(o) = role
            .opponent()
            .and_then(|o| self.unit(o).cloned())
            .filter(|o| o.slot != 0)
        else {
            return Ok(Flip::Continue);
        };
        if hi == 0xE {
            let read = self
                .units
                .iter()
                .find(|u| u.side == o.controller() && u.slot == o.slot)
                .map(|u| self.unit_card(u, cards).kind & 0x1F);
            if read != Some(0xE) {
                return Ok(Flip::Continue);
            }
        }
        let by = unit.controller();
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: Some(o.id),
        });
        if !(self.ability11(&o, cards) || o.controller() == by || self.dragon_immunity(&o, cards)) {
            self.change_control(o.id, by, events);
        }
        Ok(Flip::BattleEnds)
    }
}

impl Duel {
    /// The magic mask 0x24E360(0, 0, ctrl, mode, filter, 0xE) minus the
    /// card, then 0x273C60(mask, ctrl), in mask order.
    pub(super) fn magic_mask(
        &self,
        unit: &DuelUnit,
        mode: u32,
        filter: u32,
        cards: &impl CardData,
    ) -> Vec<UnitId> {
        let ctrl = unit.controller();
        let mut targets: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && u.id != unit.id
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        _ => true,
                    }
                    && select_filter(filter, &self.unit_card(u, cards)).unwrap_or(false)
                    && self.magic_target_admitted(u, ctrl, cards)
            })
            .collect();
        targets.sort_by_key(|u| (u.side, u.slot));
        targets.into_iter().map(|u| u.id).collect()
    }

    /// Magic type 0x21 (0x255ADC; Shield & Sword 736, `0x21DD, 0`): the
    /// magic mask, record {0x5B, 0, mask, 0} (empty: 0xFE). Event 0x5B
    /// (0x25EB50) per target: A = clamp(card ATK + change) (0x1E1510),
    /// D = the same for DEF (0x1E15F0), both read first; 0x2525C0 op 0
    /// adds D - A to the ATK change and 0x2527A0 op 0 adds A - D to the
    /// DEF change: the permanent ATK and DEF swap (terrain and auras are
    /// not part of it). [F; emu SS1]
    pub(super) fn magic_stat_swap(
        &mut self,
        unit: &DuelUnit,
        mode: u32,
        filter: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        for t in self.magic_mask(unit, mode, filter, cards) {
            let Some(u) = self.unit(t) else {
                continue;
            };
            let c = self.unit_card(u, cards);
            let a = (i32::from(c.attack) + i32::from(u.attack_bonus)).clamp(0, i32::from(MAX_LP));
            let d = (i32::from(c.defense) + i32::from(u.defense_bonus)).clamp(0, i32::from(MAX_LP));
            self.add_stat_capped(t, true, d - a, cards);
            self.add_stat_capped(t, false, a - d, cards);
            if let Some(u) = self.unit(t) {
                events.push(DuelEvent::Modified {
                    unit: t,
                    attack: u.attack_bonus,
                    defense: u.defense_bonus,
                });
            }
        }
    }
}

impl Duel {
    /// Flip type 0x33 (0x26556C; Time Wizard 36: mode 3, filter 0x37, hi
    /// 0x95F, lo 0x18): mask = 0x24E360(0, 0, ctrl, mode, 0x37 | hi << 16,
    /// 0xE); filter 0x37 (0x24F5A0) = a monster of kind 0 (Dragon, record
    /// +0x3E) whose ATK at its own cell (0x252300) is at most hi; empty →
    /// nothing; event {0x3F, 0, mask, lo} (0x264E20): per target, in mask
    /// order, 0x250C90 (graveyard, no entry 4), then 0x1E0800(its group,
    /// its cell, card lo): a fresh face-down unit in attack position, the
    /// lowest free slot, the group's next summon order. No immunity
    /// filter. [F; emu TW1: both Blackland Fire Dragons (1500) became
    /// Thousand Dragon 24 face-down, the Red-Eyes (2900 at its cell)
    /// stayed]
    pub(super) fn flip_transform(
        &mut self,
        id: UnitId,
        low: u32,
        hi: u16,
        lo: u16,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let unit = self.unit(id).expect("flipping unit").clone();
        let (mode, filter) = (low >> 6, low & 0x3F);
        if filter != 0x37 {
            return Err(unsupported(format!(
                "flip effect of card {} (type 0x33 filter {filter:#x})",
                unit.card
            )));
        }
        let ctrl = unit.controller();
        let mut targets: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && match mode {
                        1 => u.controller() == ctrl,
                        2 => self.foe(u.controller(), ctrl),
                        3 => true,
                        _ => false,
                    }
                    && self.unit_card(u, cards).kind == kind::DRAGON
                    && self.effective(u, u.position, true, cards) <= hi
            })
            .collect();
        targets.sort_by_key(|u| (u.side, u.slot));
        let targets: Vec<(UnitId, u8, GridPos)> = targets
            .into_iter()
            .map(|u| (u.id, u.side, u.position))
            .collect();
        if targets.is_empty() {
            return Ok(());
        }
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: None,
        });
        for (t, group, at) in targets {
            if self.unit(t).is_none() {
                continue;
            }
            self.remove_by_effect(t, cards, events)?;
            self.create(group, lo, at, cards, events);
        }
        Ok(())
    }
}

impl Duel {
    /// 0x24A7C0, added by the equip setter 0x249D80 to a category-0
    /// power-up's value (field equips, attacked power-ups and summon-chain
    /// equips): over every unit of group 0 then group 1 in slots 1..10 in
    /// face-up committed defence (+0x57 bits 4, 3 and 5) with an entry-2
    /// type 0x34 (45 Maha Vailo, lo 200): +lo when its hi is 0, -lo when 1.
    /// [F; emu MV1]
    pub(super) fn equip_modifier(&self, cards: &impl CardData) -> i32 {
        self.units
            .iter()
            .filter(|u| u.slot != 0 && u.committed_face_up && u.defence && u.committed_defence)
            .filter_map(|u| self.unit_card(u, cards).entries[2])
            .filter(|[w0, _]| w0 >> 8 == 0x34)
            .map(|[_, w1]| match w1 >> 16 {
                0 => (w1 & 0xFFFF) as i32,
                1 => -((w1 & 0xFFFF) as i32),
                _ => 0,
            })
            .sum()
    }
}

impl Duel {
    /// Magic type 0x27 (0x25602C; Magic Drain 748): record {0x22, ctrl ^ 1,
    /// ctrl, stars of ctrl ^ 1}; event 0x22 (0x260640): 0x1DF860(ctrl ^ 1,
    /// 4, 0) (its stars := 0), then 0x1DF860(ctrl, 0, amount) (+amount,
    /// clamped to 12). [F; emu MD1]
    pub(super) fn magic_drain(&mut self, unit: &DuelUnit, events: &mut Vec<DuelEvent>) {
        let ctrl = unit.controller();
        let opponent = usize::from(self.opp(ctrl));
        let taken = self.sides[opponent].stars;
        self.sides[opponent].stars = 0;
        let s = &mut self.sides[usize::from(ctrl)];
        s.stars = s.stars.saturating_add(taken).min(MAX_STARS);
        let _ = events;
    }

    /// Magic type 0x28 (0x256094; Earthshaker 750): record {0x23, ctrl, 0,
    /// 0}; event 0x23 (0x260860): 0x260C30 turns every Labyrinth cell into
    /// terrain 7, then 100 rounds of four rand() (0x19AE88): col = r % 7,
    /// row = r % 7, group = r & 1, slot = r % 10 + 1. An empty cell takes
    /// unit (group, slot) when it exists (0x1E0FD0: position only, no
    /// face/flag/terrain effect); an occupied cell does nothing (its swap
    /// branch needs 0x260BC0 >= 0, which only ever answers < 0; the #11
    /// test 0x2738C0 before it is a glow request). Leaders never move.
    /// [F; emu exp_earthshaker E1-E3: board, terrain and LCG state after 400
    /// draws as this model]
    pub(super) fn earthshaker(&mut self, events: &mut Vec<DuelEvent>) {
        for t in self.terrain.iter_mut() {
            if *t == terrain::LABYRINTH {
                *t = terrain::NORMAL;
            }
        }
        for _ in 0..100 {
            let column = (self.rand31() % 7) as u8;
            let row = (self.rand31() % 7) as u8;
            let side = (self.rand31() & 1) as u8;
            let slot = (self.rand31() % 10 + 1) as u8;
            let to = GridPos { column, row };
            if self.unit_at(to).is_some() {
                continue;
            }
            if let Some(id) = self
                .units
                .iter()
                .find(|u| u.side == side && u.slot == slot)
                .map(|u| u.id)
            {
                self.advance(id, to, events);
            }
        }
    }

    /// Magic type 0x49 (0x2555A8; Swords of Revealing Light 715: mode 2,
    /// filter 0x1F, lo 3): the magic mask (minus the card, 0x273C60);
    /// record {0xC, 0, mask, lo}; event 0xC (0x25B6B0) per target:
    /// 0x252FF0(g, s, 1) (revealed, committed face-up), the acted bit, then
    /// 0x251CD0(g, s, lo) (bound lo turns). [F; emu SR1]
    pub(super) fn magic_reveal_bind(
        &mut self,
        unit: &DuelUnit,
        mode: u32,
        filter: u32,
        lo: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        for t in self.magic_mask(unit, mode, filter, cards) {
            self.expose(t, events);
            if let Some(u) = self.units.iter_mut().find(|u| u.id == t) {
                u.acted = true;
            }
            self.bind(t, lo as u8, events);
        }
    }
}

impl Duel {
    /// Flip type 4 with a scope (0x265D0C; hi != 0): mask = 0x24E360(col,
    /// row, ctrl, mode, filter, scope hi) around the unit's own cell (see
    /// [`Duel::scope_mask`]); 652 Man Eater (filter 0x1F, hi 0x14 Forest),
    /// 629 Dissolverock (0x16 Mountain), 115 Fire Reaper (0x17 Meadow), 118
    /// Flame Ghost (0x18 Sea), 98 Hurricail (filter 0x1E, hi 1: non-monster
    /// cards within Manhattan distance 1). Event {0x15, 0, mask, 0}: every
    /// unit of the mask to the graveyard (0x250C90, no entry 4), the
    /// flipping unit included when it is listed. In battle (role > 0) the
    /// result is 0x201 when the opponent is in the mask, else 0x401 when
    /// the unit itself is. [F; emu ME1, REFUSALS-SWEEP.md FG1/FG2/FR1/HC1]
    pub(super) fn flip_destroy_on_terrain(
        &mut self,
        id: UnitId,
        role: FlipRole,
        low: u32,
        hi: u16,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Flip> {
        let unit = self.unit(id).expect("flipping unit").clone();
        let (mode, filter) = (low >> 6, low & 0x3F);
        // 0x265D14: hi 0 → 0x24E360(0, 0, ctrl, mode, filter, 0xE).
        let (center, scope) = if hi == 0 {
            (GridPos { column: 0, row: 0 }, 0xE)
        } else {
            (unit.position, hi)
        };
        let mask = self
            .scope_mask(center, unit.controller(), mode, filter, scope, cards)
            .map_err(|_| {
                unsupported(format!(
                    "flip effect of card {} (type 4 filter {filter:#x} scope {hi:#x})",
                    unit.card
                ))
            })?;
        if mask.is_empty() {
            return Ok(Flip::Continue);
        }
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: None,
        });
        for &t in &mask {
            if self.unit(t).is_some() {
                self.remove_by_effect(t, cards, events)?;
            }
        }
        Ok(match role.opponent() {
            Some(o) if mask.contains(&o) => Flip::OpponentDestroyed,
            Some(_) if mask.contains(&id) => Flip::SelfDestroyed,
            _ => Flip::Continue,
        })
    }
}

impl Duel {
    /// 0x24E360(col, row, ctrl, mode, filter, scope) for the scopes the
    /// engine knows, in mask order (g0 s1..s10, g1 s1..s10). The candidates
    /// are 0x24E230(ctrl, mode): slots 1..10 of the mode's controllers, any
    /// face, the caller included. Scopes [F 0x24E3F8..0x24EB80]:
    /// - 0x14 / 0x16 / 0x17 / 0x18: units standing on terrain 0 (Forest) /
    ///   2 (Mountain) / 3 (Meadow) / 4 (Sea) (0x1E3530); filters 0x34 and
    ///   0x1F take any such unit, 0x1D a monster whose controller fits the
    ///   mode (0x24F6A0), every other filter nothing;
    /// - 0x12 and 0x1D: not traced (refused);
    /// - any other value: units within Manhattan distance `scope` of (col,
    ///   row) that pass the filter jump table 0x2D3130 ([`select_filter`]);
    ///   scope 0xE covers the whole board.
    pub(super) fn scope_mask(
        &self,
        center: GridPos,
        ctrl: u8,
        mode: u32,
        filter: u32,
        scope: u16,
        cards: &impl CardData,
    ) -> Result<Vec<UnitId>> {
        let ground = match scope {
            0x14 => Some(terrain::FOREST),
            0x16 => Some(terrain::MOUNTAIN),
            0x17 => Some(terrain::MEADOW),
            0x18 => Some(terrain::SEA),
            0x12 | 0x1D => return Err(unsupported(format!("target scope {scope:#x}"))),
            _ => None,
        };
        let mut mask: Vec<&DuelUnit> = Vec::new();
        for u in &self.units {
            let listed = u.slot != 0
                && match mode {
                    1 => u.controller() == ctrl,
                    2 => self.foe(u.controller(), ctrl),
                    3 => true,
                    _ => false,
                };
            if !listed {
                continue;
            }
            let c = self.unit_card(u, cards);
            let hit = match ground {
                Some(t) => {
                    self.terrain_at(u.position) == t
                        && match filter {
                            0x34 | 0x1F => true,
                            0x1D => c.is_monster(),
                            _ => false,
                        }
                }
                None => {
                    u32::from(u.position.column.abs_diff(center.column))
                        + u32::from(u.position.row.abs_diff(center.row))
                        <= u32::from(scope)
                        && select_filter(filter, &c)?
                }
            };
            if hit {
                mask.push(u);
            }
        }
        mask.sort_by_key(|u| (u.side, u.slot));
        Ok(mask.into_iter().map(|u| u.id).collect())
    }
}

impl Duel {
    /// Entry-4 type 0x24 (0x25346C; The Wicked Worm Beast 228, mode/filter
    /// unread): record {0x1E, 0, bit(dying unit), ctrl}; event 0x1E
    /// (0x25F790), per unit of the mask (its group g = physical owner):
    /// 1. 0x1DFCB0(g, card): with fewer than 40 cards (S+0x230), draws
    ///    0x19AE88 until slot floor(r * 40 / 2^31) of the 40-slot deck
    ///    array S+0x1E0 is empty (999) and writes the card there, count +1;
    ///    a full deck takes nothing.
    /// 2. 0x1DFDA0(0): compacts group **0**'s deck only (a group-1 card
    ///    keeps its gap until that side's next draw, 0x1DFFB8, compacts it:
    ///    the card then sits after the other cards, as here).
    /// 3. 0x1E0E30(g, s): the unit leaves without the graveyard.
    ///
    /// A group-1 card written past the compacted cards waits in
    /// `Duel::deck_stranded` until that group's next successful draw; past
    /// slot 0 of an empty deck it is never drawn (the draw returns 999
    /// without compacting) until a later return fills slot 0
    /// (sweep_rules.rs). [F; emu WW1: card 228 written to slot 29 of group 1's
    /// 25-card deck after two draws, count 26, the unit gone, no graveyard;
    /// the slot formula matched 0x1DFCB0 in 400 interpreter runs]
    pub(super) fn return_to_deck(
        &mut self,
        unit: &DuelUnit,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Aftermath> {
        let g = unit.side;
        events.push(DuelEvent::Activated {
            unit: unit.id,
            card: unit.card,
            target: None,
        });
        if self.deck_count(g) < DECK_SIZE {
            // 0x1DFCB0: rand31 → slot floor(r * 40 / 2^31) until it is 999.
            let slot = loop {
                let r = u64::from(self.rand31());
                let slot = ((r * DECK_SIZE as u64) >> 31) as usize;
                if self.deck_slot(g, slot) == sweep_rules::EMPTY_SLOT {
                    break slot;
                }
            };
            self.place_in_deck(g, slot as u8, unit.card);
        }
        // 0x1DFDA0(0): group 0's deck array is compacted.
        self.compact_deck(0);
        let _ = cards;
        if let Some(i) = self.units.iter().position(|u| u.id == unit.id) {
            self.units.remove(i);
            // Presentation stand-in: there is no "back to the deck" event;
            // `Destroyed` is the closest (the card skips the graveyard).
            events.push(DuelEvent::Destroyed {
                unit: unit.id,
                card: unit.card,
            });
        }
        Ok(Aftermath::Continue)
    }
}

impl Duel {
    /// A compatible attacker attacks a Riryoku (789, category 0, value 0):
    /// event {0x25, power-up, attacker, its group << 16 | card} (0x26B9D0).
    /// The handler shows the attacker (committing an exposed one, as event
    /// 0x24), computes amount = 0x24A750(group, 0x24A7C0()) =
    /// modifier(LP of group ^ 1 >> 1) + the type-0x34 sum, where group is
    /// the **Riryoku's** group: the attacker's own side pays. Without a
    /// trap, 0x249D80 adds the amount to the attacker's ATK and DEF and
    /// 0x1E0710(group ^ 1, 1, amount) takes it from that side's LP. Leader
    /// ability #11 at the attacker's cell (0x2738C0) skips the trap search;
    /// the Goblin Fan (0x29) and Reverse Trap (0x2B) answers are below.
    /// [F; emu RR1/RR2: Red-Eyes +1350, side 0 LP 2700 ->
    /// 1350 per application; the harness's repeated sub-script phase 2
    /// applied it four times]
    pub(super) fn riryoku_attacked(
        &mut self,
        att: UnitId,
        power: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let attacker = self.unit(att).expect("attacker").clone();
        let group = self.unit(power).expect("power-up").side;
        // 0x26BB6C: leader ability #11 at the attacker's cell skips the
        // trap searches. Else 0x26C4B0(group, 0x29): a Goblin Fan of the
        // attacker's side (mask 2: the Riryoku owner's opponents), then
        // 0x26C4B0(attacker's group, 0x2B): a Reverse Trap of either group
        // (mask 3, the attacker's group first). A found trap is revealed
        // and acted and stays (sub-script 0x25A). [emu REFUSALS-SWEEP.md
        // RT1-RT4: the Riryoku owner's own Goblin Fan does nothing; the
        // attacker's Fan makes the Riryoku's side pay the amount once and
        // the attacker gains nothing; a Reverse Trap of either side turns
        // the gain into a loss (0x2525C0 / 0x2527A0 op 1, floored at the
        // card's base) while the attacker's side still pays.] The Royal
        // Decree query that follows a found trap is below.
        let (fan, reverse) = if self.ability11(&attacker, cards) {
            (None, None)
        } else {
            let fan = self.far_trap_in(&self.foes(group), trap_type::GOBLIN_FAN, cards);
            let reverse = if fan.is_none() {
                self.far_trap_in(&self.first_then_all(attacker.side), 0x2B, cards)
            } else {
                None
            };
            (fan, reverse)
        };
        // 0x26BBA4 / 0x26BCA0: 0x26C4B0(rec[8], 0x2E) passes the attacker's
        // unit word (group << 8 | slot) as the group of 0x26C5C0's mask-3
        // search: first "group" word (group 1 only for g0 s1, group 0 for
        // the word 0; any larger word reads past the two groups [H: no
        // trap there]), then group (word != 0 ? 0 : 1). A Decree found
        // reacts and the Riryoku resolves as without the trap [emu
        // REFUSALS-SWEEP.md RD1/RD4: a group-0 Decree took over from the
        // Fan / Reverse Trap; RD2/RD3: a group-1 Decree was not seen by a
        // g0 s3 attacker].
        let decree = if fan.is_some() || reverse.is_some() {
            let word = u16::from(attacker.side) << 8 | u16::from(attacker.slot);
            let groups: &[u8] = match word {
                0 => &[0, 1],
                1 => &[1, 0],
                _ => &[0],
            };
            self.far_trap_in(groups, trap_type::ROYAL_DECREE, cards)
        } else {
            None
        };
        let (fan, reverse) = if decree.is_some() { (None, None) } else { (fan, reverse) };
        if let Some(d) = decree {
            self.react_trap(d, Some(att), events);
        }
        if let Some(u) = self.units.iter_mut().find(|u| u.id == att) {
            u.committed_face_up |= u.face_up;
            u.face_up = u.committed_face_up;
        }
        let amount = self.riryoku_amount(group, cards);
        if let Some(trap) = fan {
            self.react_trap(trap, Some(att), events);
            self.damage(group, amount, events);
        } else if let Some(trap) = reverse {
            self.react_trap(trap, Some(att), events);
            self.lower_stats(att, i32::from(amount), cards, events);
            self.damage(self.opp(group), amount, events);
        } else {
            self.boost_capped(att, i32::from(amount), cards, events);
            self.damage(self.opp(group), amount, events);
        }
        Ok(())
    }
}
