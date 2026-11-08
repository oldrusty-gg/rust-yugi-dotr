//! The EE-side view of a duel that Weevil's code reads, and the shared
//! helpers (0x1Exxxx accessors, 0x25xxxx stat queries, 0x27xxxx evaluators)
//! it calls. Every function keeps the original's argument order, sentinel
//! values (−1 for "no unit", 999 for "no card") and quirks; names carry the
//! EE address.
//!
//! Scope: the helpers are exact for states the duel engine accepts. Where an
//! original branch depends on mechanics the engine refuses (leader
//! abilities other than #0..#4/#6/#9/#10/#12, continuous effects, control changes) the
//! helper reproduces the count/scan faithfully and returns
//! [`DuelError::Unsupported`] instead of guessing the effect.

use super::FusionData;
use crate::duel::{CardData, CardInfo, Duel, DuelError};
use crate::UnitId;

pub(super) type R<T> = Result<T, DuelError>;

pub(super) fn unsupported<T>(what: impl Into<String>) -> R<T> {
    Err(DuelError::Unsupported(what.into()))
}

/// Unit key as the EE passes it: (group << 8) | slot.
pub(super) type Key = u16;

pub(super) const fn key(group: u8, slot: u8) -> Key {
    (group as u16) << 8 | slot as u16
}

/// Flag bits of unit +0x57.
pub(super) mod flag {
    pub const ACTED: u8 = 0;
    pub const SWAPPED: u8 = 1;
    pub const COMMITTED_FACE_UP: u8 = 4;
    pub const COMMITTED_DEFENCE: u8 = 5;
}

/// The unit-struct fields the AI reads (0x2DB990 + group·0x3F4 + slot·0x5C,
/// or a virtual record built by 0x1DDFE0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Unit {
    /// +0x3C.
    pub card: u16,
    /// +0x3E card byte 0 (class << 5 | kind).
    pub kind: u8,
    /// +0x3F card byte 2 & 0xF.
    pub attribute: u8,
    /// +0x40/+0x41; 7/7 for virtual records.
    pub col: u8,
    pub row: u8,
    /// +0x44 owner (2 for virtual records).
    pub side: u8,
    /// +0x45 (0xB for virtual records).
    pub slot: u8,
    /// +0x47.
    pub order: u8,
    /// +0x48, subtracted by the original level getter 0x1E1B70.
    pub discount: u8,
    /// +0x49.
    pub spellbind: u8,
    /// +0x4A (never set by the engine).
    pub x4a: u8,
    /// +0x57.
    pub flags: u8,
    /// +0x38/+0x3A.
    pub atk_mod: i16,
    pub def_mod: i16,
    /// +0x08..+0x2F: effect entries (word0, word1); `None` = both 0xFFFFFFFF.
    pub entries: [Option<[u32; 2]>; 5],
    /// The engine unit (None for virtual records).
    pub id: Option<UnitId>,
}

impl Unit {
    pub fn bit(&self, n: u8) -> bool {
        self.flags >> n & 1 == 1
    }
    pub fn class(&self) -> u8 {
        self.kind >> 5
    }
    /// 0x251C90: controlling side (owner, swapped by +0x57 bit1).
    pub fn ctrl(&self) -> i32 {
        if self.bit(flag::SWAPPED) {
            i32::from(self.side == 0)
        } else {
            i32::from(self.side)
        }
    }
    /// 0x252220: a leader of side 0 or 1.
    pub fn is_leader(&self) -> bool {
        self.slot == 0 && self.side < 2
    }
    /// Effect entry `n` (0x1E1DF0 / 0x252060).
    pub fn entry(&self, n: usize) -> Option<[u32; 2]> {
        self.entries[n]
    }
    /// Entry type (word0 >> 8) of entry `n`, 0xFFFFFF when absent (the
    /// original shifts the sentinel word).
    pub fn entry_type(&self, n: usize) -> u32 {
        self.entries[n].map_or(0xFF_FFFF, |[w, _]| w >> 8)
    }
    pub fn pos(&self) -> i32 {
        i32::from(self.row) << 8 | i32::from(self.col)
    }
}

/// 0x2505F0(u, source controller, mode, filter): whether a continuous
/// effect with entry low byte `low` (mode << 6 | filter) applies to `u`.
/// Monsters only; mode via 0x24F6A0 (1 same controller, 2 the other, 3
/// both, 0 none); filters 2, 6, 7, 0xE, 0xF, 0x10, 0x11, 0x13 (kind),
/// 0x14, 0x15, 0x19 (attribute 0, 1, 5), 0x1D (any); 0x31 (fits power-up
/// 0x2FF) is not ported and any other filter never applies.
fn aura_applies(u: &Unit, source: i32, low: u32) -> bool {
    if u.class() != 0 {
        return false;
    }
    let side_ok = match low >> 6 {
        3 => true,
        2 => u.ctrl() == i32::from(source == 0),
        1 => u.ctrl() == source,
        _ => false,
    };
    let kind = u32::from(u.kind & 0x1F);
    let filter_ok = match low & 0x3F {
        f @ (2 | 6 | 7 | 0xE | 0xF | 0x10 | 0x11 | 0x13) => kind == f,
        0x14 => u.attribute == 0,
        0x15 => u.attribute == 1,
        0x19 => u.attribute == 5,
        0x1D => true,
        _ => false,
    };
    side_ok && filter_ok
}

pub(super) struct World<'a, C: CardData + ?Sized, F: FusionData + ?Sized> {
    pub duel: &'a Duel,
    pub cards: &'a C,
    pub fusions: &'a F,
    /// Groups 0 and 1, slots 0..=10.
    pub units: [[Option<Unit>; 11]; 2],
    /// Unit (group, slot) on each cell (cell +0 pointer).
    pub cells: [Option<(u8, u8)>; 49],
    /// 0x2DB8F4.
    pub active: u8,
    /// s16 0x29922E read through 0x23BCD0.
    pub tier: i16,
    /// The shared LCG state of rand() 0x19AE88 (`Duel::rng`), advanced by
    /// the modules that draw from it ([`World::rand31`]).
    pub rng: std::cell::Cell<u64>,
}

fn flags_of(u: &crate::duel::DuelUnit) -> u8 {
    u8::from(u.acted)
        | u8::from(u.stolen) << 1
        | u8::from(u.face_up) << 2
        | u8::from(u.defence) << 3
        | u8::from(u.committed_face_up) << 4
        | u8::from(u.committed_defence) << 5
}

impl<'a, C: CardData + ?Sized, F: FusionData + ?Sized> World<'a, C, F> {
    pub fn new(duel: &'a Duel, cards: &'a C, fusions: &'a F, tier: i16) -> R<Self> {
        let mut units: [[Option<Unit>; 11]; 2] = Default::default();
        let mut cells = [None; 49];
        for side in &duel.sides {
            // [F] Every ability read reachable from the CPU modules
            // (0x10D210..0x172DD4 and their callees, through 0x273070 /
            // 0x2738C0 / 0x273770) is modelled or refused where it is made:
            // #4 / #12 (0x247E10, `allowance`), #8 (0x273160), #9 / #10 /
            // #14 (0x273370 / 0x273420), #11 (0x2738C0 in 0x24CC30, 0x24DDA0
            // and 0x26CB20: [`Self::leader_guard`]; 0x273770 in 0x273B40),
            // #13 / #15 / #16 (0x24CC30's on-entry branch: refused by
            // `plans::near_trap`). #0..#3, #5, #6 and #7 are never read.
            if side.leader_abilities.iter().any(|&a| a > 16) {
                return unsupported("a leader ability above #16");
            }
        }
        for u in &duel.units {
            if u.card == 0x29F {
                // 0x1DDFE0 stores 0x29F as card 999 without a record.
                return unsupported("card 0x29F on the field");
            }
            cards
                .card(u.card)
                .ok_or_else(|| DuelError::Unsupported(format!("unknown card {}", u.card)))?;
            if u.side > 1 || u.slot > 10 {
                return Err(DuelError::InvalidCommand("unit outside the 2x11 table"));
            }
            let cell = u
                .position
                .index()
                .ok_or(DuelError::InvalidCommand("unit off the board"))?;
            // The unit record: kind, attribute and entries as duel effects
            // rewrote them (flip types 0x36/0x37, category-1 power-ups).
            let rec = duel.unit_card(u, cards);
            let unit = Unit {
                card: u.card,
                kind: rec.kind,
                attribute: rec.attribute,
                col: u.position.column,
                row: u.position.row,
                side: u.side,
                slot: u.slot,
                order: u.summon_order,
                discount: u.discount,
                spellbind: u.spellbind,
                // +0x4A (0x1E28C0): Binding Chain's continuous bind.
                x4a: duel.chain_bound.get(&u.id.0).copied().unwrap_or(0),
                flags: flags_of(u),
                atk_mod: u.attack_bonus,
                def_mod: u.defense_bonus,
                entries: record_entries(&rec),
                id: Some(u.id),
            };
            units[usize::from(u.side)][usize::from(u.slot)] = Some(unit);
            cells[cell] = Some((u.side, u.slot));
        }
        Ok(Self {
            duel,
            cards,
            fusions,
            units,
            cells,
            active: duel.active,
            tier,
            rng: std::cell::Cell::new(duel.rng),
        })
    }

    /// rand() 0x19AE88: one draw from the shared LCG
    /// ([`crate::duel::next_rand31`]).
    pub fn rand31(&self) -> u32 {
        let mut state = self.rng.get();
        let r = crate::duel::next_rand31(&mut state);
        self.rng.set(state);
        r
    }

    pub fn card(&self, id: u16) -> Option<&'a CardInfo> {
        self.cards.card(id)
    }

    // ---- 0x1Exxxx accessors -------------------------------------------

    /// 0x1E07C0 (0x1DE3A0 + slot check): the unit in (group, slot).
    pub fn unit(&self, g: i32, s: i32) -> Option<&Unit> {
        if !(0..2).contains(&g) || !(0..11).contains(&s) {
            return None;
        }
        self.units[g as usize][s as usize].as_ref()
    }
    pub fn unit_k(&self, k: i32) -> Option<&Unit> {
        self.unit(k >> 8, k & 0xFF)
    }
    /// 0x1E3380: bit `n` of +0x57, false without a unit.
    pub fn flag(&self, g: i32, s: i32, n: u8) -> bool {
        self.unit(g, s).is_some_and(|u| u.bit(n))
    }
    /// 0x24F790: controller of (g, s): g, or the other side when swapped.
    pub fn ctrl(&self, g: i32, s: i32) -> i32 {
        if self.flag(g, s, flag::SWAPPED) {
            i32::from(g == 0)
        } else {
            g
        }
    }
    /// 0x1E2310: (row << 8) | col, −1 without a unit.
    pub fn pos(&self, g: i32, s: i32) -> i32 {
        if g == 2 {
            // 0x1DE3A0 accepts group 2 (0x2DC178 + 0x5C·slot, slots 0..=10):
            // the temporary records of the hand display, whose cell the
            // duel carries ([`Duel::temp_units`]); a reset record has slot
            // 0xB and gives −1 like a missing unit.
            return usize::try_from(s)
                .ok()
                .and_then(|s| self.duel.temp_units.get(s).copied().flatten())
                .map_or(-1, |t| {
                    i32::from(t.cell.row) << 8 | i32::from(t.cell.column)
                });
        }
        self.unit(g, s).map_or(-1, Unit::pos)
    }
    /// 0x1E1980: +0x3E, −1 without a unit.
    pub fn kind(&self, g: i32, s: i32) -> i32 {
        self.unit(g, s).map_or(-1, |u| i32::from(u.kind))
    }
    /// 0x1E22B0: card id, −1 without a unit.
    pub fn card_id(&self, g: i32, s: i32) -> i32 {
        self.unit(g, s).map_or(-1, |u| i32::from(u.card))
    }
    /// 0x1E2640: summon order +0x47, −1 without a unit.
    pub fn order(&self, g: i32, s: i32) -> i32 {
        self.unit(g, s).map_or(-1, |u| i32::from(u.order))
    }
    /// 0x1E2800: spellbind +0x49, −1 without a unit.
    pub fn spellbind(&self, g: i32, s: i32) -> i32 {
        self.unit(g, s).map_or(-1, |u| i32::from(u.spellbind))
    }
    /// 0x1E28C0: +0x4A, −1 without a unit.
    pub fn x4a(&self, g: i32, s: i32) -> i32 {
        self.unit(g, s).map_or(-1, |u| i32::from(u.x4a))
    }
    fn cell_index(c: i32, r: i32) -> Option<usize> {
        ((0..7).contains(&c) && (0..7).contains(&r)).then(|| (r * 7 + c) as usize)
    }
    /// 0x1E4960: the unit on a cell.
    pub fn at(&self, c: i32, r: i32) -> Option<&Unit> {
        let (g, s) = self.cells[Self::cell_index(c, r)?]?;
        self.unit(i32::from(g), i32::from(s))
    }
    /// 0x1E4A10: (owner +0x44 << 8) | slot of the unit on a cell, or −1.
    pub fn at_key(&self, c: i32, r: i32) -> i32 {
        self.at(c, r)
            .map_or(-1, |u| i32::from(u.side) << 8 | i32::from(u.slot))
    }
    /// 0x1E3530: terrain byte, −1 off the board.
    pub fn terrain(&self, c: i32, r: i32) -> i32 {
        Self::cell_index(c, r).map_or(-1, |i| i32::from(self.duel.terrain[i]))
    }
    /// 0x1E13D0: units in slots 1..=5 of group `g`.
    pub fn monsters(&self, g: i32) -> i32 {
        (1..6).filter(|&s| self.unit(g, s).is_some()).count() as i32
    }
    /// 0x1E1470: units in slots 6..=10 of group `g`.
    pub fn others(&self, g: i32) -> i32 {
        (6..11).filter(|&s| self.unit(g, s).is_some()).count() as i32
    }
    fn side_state(&self, side: i32) -> Option<&'a crate::duel::SideState> {
        self.duel.sides.get(usize::try_from(side).ok()?)
    }
    /// 0x1E06F0: LP.
    pub fn lp(&self, side: i32) -> i32 {
        self.side_state(side).map_or(0, |s| i32::from(s.lp))
    }
    /// 0x1DF840: stars.
    pub fn stars(&self, side: i32) -> i32 {
        self.side_state(side).map_or(0, |s| i32::from(s.stars))
    }
    /// 0x1DF950: summon still allowed this turn.
    pub fn can_summon(&self, side: i32) -> bool {
        self.side_state(side).is_some_and(|s| s.can_summon)
    }
    /// 0x1E0340: hand slot `i` (999 empty); the engine keeps the hand
    /// compacted, which is how the EE leaves it between summons.
    pub fn hand(&self, side: i32, i: i32) -> i32 {
        if !(0..5).contains(&i) {
            return -1;
        }
        self.side_state(side)
            .and_then(|s| s.hand.get(i as usize))
            .map_or(999, |&c| i32::from(c))
    }
    /// 0x1E0390: hand count.
    pub fn hand_count(&self, side: i32) -> i32 {
        self.side_state(side).map_or(0, |s| s.hand.len() as i32)
    }
    /// 0x1DFC60: deck entry `i` (next card first).
    pub fn deck(&self, side: i32, i: i32) -> i32 {
        if !(0..40).contains(&i) {
            return -1;
        }
        match u8::try_from(side) {
            Ok(g @ 0..=1) => i32::from(self.duel.deck_slot(g, i as usize)),
            _ => 999,
        }
    }
    /// 0x1DFC40: deck count.
    pub fn deck_count(&self, side: i32) -> i32 {
        match u8::try_from(side) {
            Ok(g @ 0..=1) => self.duel.deck_count(g) as i32,
            _ => 0,
        }
    }
    /// 0x1E00D0: graveyard count (side +0x322).
    pub fn graveyard_count(&self, side: i32) -> i32 {
        self.side_state(side)
            .map_or(0, |s| s.graveyard.len() as i32)
    }
    /// 0x1E0080(side, i): graveyard entry `i` (u16 side +0x232), 999 past
    /// the entries the state holds (the engine pads the table with 999).
    pub fn graveyard(&self, side: i32, i: i32) -> i32 {
        if !(0..120).contains(&i) {
            return -1;
        }
        self.side_state(side)
            .and_then(|s| s.graveyard.get(i as usize))
            .map_or(999, |&c| i32::from(c))
    }

    // ---- stats ----------------------------------------------------------

    fn record(&self, u: &Unit) -> R<&'a CardInfo> {
        self.card(u.card)
            .ok_or_else(|| DuelError::Unsupported(format!("unknown card {}", u.card)))
    }
    /// 0x1E1570: printed ATK, plus +0x38 unless `raw`, clamped to 0..=9999.
    pub fn atk_of(&self, u: &Unit, raw: bool) -> R<i32> {
        let mut v = i32::from(self.record(u)?.attack & 0x1FFF);
        if !raw {
            v += i32::from(u.atk_mod);
        }
        Ok(v.clamp(0, 9999))
    }
    /// 0x1E1650: printed DEF, plus +0x3A unless `raw`, clamped to 0..=9999.
    pub fn def_of(&self, u: &Unit, raw: bool) -> R<i32> {
        let mut v = i32::from(self.record(u)?.defense & 0x1FFF);
        if !raw {
            v += i32::from(u.def_mod);
        }
        Ok(v.clamp(0, 9999))
    }
    /// 0x1E1510 (−1 without a unit).
    pub fn atk(&self, g: i32, s: i32, raw: bool) -> R<i32> {
        match self.unit(g, s) {
            Some(u) => self.atk_of(u, raw),
            None => Ok(-1),
        }
    }
    /// 0x1E15F0 (−1 without a unit).
    pub fn def(&self, g: i32, s: i32, raw: bool) -> R<i32> {
        match self.unit(g, s) {
            Some(u) => self.def_of(u, raw),
            None => Ok(-1),
        }
    }

    /// 0x24F8A0: 1 favoured, 2 unfavoured, 0 neutral.
    pub fn terrain_favour(&self, terrain: i32, u: &Unit) -> R<i32> {
        let Ok(t) = u8::try_from(terrain) else {
            return Ok(0);
        };
        if t > 9 {
            return Ok(0);
        }
        Ok(match crate::duel::terrain_affinity(t, self.record(u)?) {
            1 => 1,
            -1 => 2,
            _ => 0,
        })
    }
    /// 0x24F7D0: 0 for non-monsters and leaders, else 0x24F8A0.
    pub fn terrain_bonus_kind(&self, terrain: i32, u: &Unit) -> R<i32> {
        if u.class() != 0 || u.slot == 0 {
            return Ok(0);
        }
        self.terrain_favour(terrain, u)
    }
    /// 0x24F820(terrain, g, s).
    pub fn terrain_favour_of(&self, terrain: i32, g: i32, s: i32) -> R<i32> {
        if self.kind(g, s) >> 5 != 0 || s == 0 {
            return Ok(0);
        }
        match self.unit(g, s) {
            Some(u) => self.terrain_favour(terrain, u),
            None => Ok(0),
        }
    }

    /// 0x268710: units in slots 1..=10 (group 0 then 1) with +0x57 bits 4,
    /// 3 and 5 set and effect entry 2 of type `ty` (continuous effects).
    pub fn continuous_count(&self, ty: u32) -> i32 {
        let mut n = 0;
        for g in 0..2 {
            for s in 1..11 {
                if let Some(u) = self.unit(g, s) {
                    if u.entry(2).is_some()
                        && u.bit(4)
                        && u.bit(3)
                        && u.bit(5)
                        && u.entry_type(2) == ty
                    {
                        n += 1;
                    }
                }
            }
        }
        n
    }
    /// 0x268890: as 0x268710 but only units controlled by `side`.
    pub fn continuous_count_of(&self, ty: u32, side: i32) -> i32 {
        let mut n = 0;
        for g in 0..2 {
            for s in 1..11 {
                if self.flag(g, s, 4)
                    && self.flag(g, s, 3)
                    && self.flag(g, s, 5)
                    && self.ctrl(g, s) == side
                {
                    if let Some(u) = self.unit(g, s) {
                        if u.entry(2).is_some() && u.entry_type(2) == ty {
                            n += 1;
                        }
                    }
                }
            }
        }
        n
    }
    /// 0x268590: key of the `n`-th (from 0) continuous unit of type `ty`
    /// (group 0 slots 1..=10 as slot, group 1 as 0x100 | slot), or −1.
    pub fn continuous_nth(&self, ty: u32, n: i32) -> i32 {
        let mut seen = 0;
        for g in 0..2 {
            for s in 1..11 {
                if self.flag(g, s, 4) && self.flag(g, s, 3) && self.flag(g, s, 5) {
                    let u = self.unit(g, s).expect("flagged unit");
                    if u.entry(2).is_some() && u.entry_type(2) == ty {
                        if seen == n {
                            return g << 8 | s;
                        }
                        seen += 1;
                    }
                }
            }
        }
        -1
    }

    /// 0x273070(n, side): leader ability `n` of `side`, suppressed while the
    /// opponent controls a continuous type-0x58 unit.
    pub fn ability(&self, n: u8, side: i32) -> bool {
        if self.continuous_count_of(0x58, i32::from(side == 0)) > 0 {
            return false;
        }
        // 0x1E1E80(side, 0, n, 0): the leader's ability table and rank.
        self.side_state(side)
            .is_some_and(|s| s.leader_abilities.contains(&n))
    }
    /// [`Self::ability`] as the aura checks reach it with the owner of a
    /// hand card's virtual record (side 2): 0x273070(n, 2) first asks
    /// 0x268890(0x58, 0) (`movn` at 0x273094: side 0 for any side ≠ 0),
    /// then 0x1E1E80(2, 0, n, 0) tests group-2 slot 0
    /// ([`crate::duel::Duel::temp_units`]) [F 0x273070, 0x1E1E80, 0x1E1F00].
    /// A reset record (slot 0xB) is null (0); the 999 filler fails the card
    /// test (≥ 0x356). A real card there (a lifted field unit's copy, state
    /// 0x44, or a fusion result 0x1D9F50 re-created in slot 0) is read like
    /// any record: option 0x1C's bit for its +0x44, then the case table
    /// over its +0x08..+0x2F halfwords at rank 0x2154C0(+0x44, card)
    /// ([`crate::duel::queries::record_ability`], [`Duel::stored_rank`]).
    pub fn ability_r(&self, n: u8, side: i32) -> R<bool> {
        if side == 2 {
            if self.continuous_count_of(0x58, 0) > 0 {
                return Ok(false);
            }
            return Ok(self.duel.temp_units[0].is_some_and(|t| self.copy_ability(&t, n)));
        }
        Ok(self.ability(n, side))
    }
    /// 0x1E1EF0(record, n, 0) on a group-2 record.
    fn copy_ability(&self, t: &crate::duel::TempUnit, n: u8) -> bool {
        if t.card >= 0x356 || t.card == 0x29F || n > 16 {
            return false;
        }
        if self.duel.ability_option.unwrap_or(3) & (1u32 << (t.side & 31)) == 0 {
            return false;
        }
        let Some(card) = self.card(t.card) else {
            return false;
        };
        // A 0x1DDFE0 record: entry 0 only for non-monsters.
        let mut info = card.clone();
        info.entries = t.entries.unwrap_or_else(|| record_entries(card));
        crate::duel::queries::record_ability(
            &crate::duel::queries::entry_halfwords(&info),
            n,
            self.duel.stored_rank(t.side, t.card),
        )
    }
    /// 0x273370(side, col, row): the cell lies in the clipped square around
    /// `side`'s leader (0x1E2310(side, 0), −1 → (255, −1): never), half-size
    /// 2 with ability #9 and 1 otherwise (0x273280; ABILITY-9-10.md §2).
    pub fn leader_window(&self, side: i32, c: i32, r: i32) -> R<bool> {
        let p = self.pos(side, 0);
        let half = if self.ability_r(9, side)? { 2 } else { 1 };
        let (lc, lr) = (p & 0xFF, p >> 8);
        Ok((lc - half).max(0) <= c
            && c <= (lc + half).min(6)
            && (lr - half).max(0) <= r
            && r <= (lr + half).min(6))
    }
    /// 0x1E1910(side, 0): the leader card's byte 0 (from the card record,
    /// never the unit's +0x3E), −1 without a leader.
    pub fn leader_kind(&self, side: i32) -> i32 {
        if side == 2 {
            // Group-2 slot 0 through 0x1DE3A0 (+0x45 < 0xB): its card
            // pointer's byte 0 [F 0x1E1910]; −1 for a reset record and,
            // as before, for 999 (only read after 0x1E1EF0 said yes).
            return self
                .duel
                .temp_units[0]
                .and_then(|t| (t.card < 0x356).then(|| self.card(t.card)).flatten())
                .map_or(-1, |c| i32::from(c.kind));
        }
        self.unit(side, 0)
            .and_then(|u| self.card(u.card))
            .map_or(-1, |c| i32::from(c.kind))
    }
    /// 0x1E1B70(side, 0, 0): clamp(level − leader +0x48, 0, 12), −1
    /// without a leader.
    fn leader_level(&self, side: i32) -> i32 {
        if side == 2 {
            // 0x1E1BD0 on group-2 slot 0: card level − +0x48, clamped.
            return self
                .duel
                .temp_units[0]
                .and_then(|t| {
                    (t.card < 0x356)
                        .then(|| self.card(t.card))
                        .flatten()
                        .map(|c| (i32::from(c.level) - i32::from(t.discount)).clamp(0, 12))
                })
                .unwrap_or(-1);
        }
        self.unit(side, 0)
            .and_then(|u| self.card(u.card).map(|c| (c, u.discount)))
            .map_or(-1, |(c, discount)| {
                (i32::from(c.level) - i32::from(discount)).clamp(0, 12)
            })
    }
    /// 0x273830(pos, side, kind, n): `pos` (row << 8 | col) in `side`'s
    /// leader window, the leader has ability `n` and its card's kind & 0x1F
    /// equals `kind` (abilities #10 and #12; ABILITY-12.md §2).
    pub fn leader_kind_aura(&self, pos: i32, side: i32, kind: i32, n: u8) -> R<bool> {
        Ok(self.leader_window(side, pos & 0xFF, pos >> 8)?
            && self.ability_r(n, side)?
            && self.leader_kind(side) & 0x1F == kind)
    }
    /// 0x273770(g, s, n): [`Self::leader_kind_aura`] for the unit (g, s)
    /// at its own cell, with its controller's leader (0x1E2310, 0x24F790,
    /// 0x1E1980 & 0x1F).
    pub fn leader_kind_aura_of(&self, g: i32, s: i32, n: u8) -> R<bool> {
        let p = self.pos(g, s);
        let ctrl = self.ctrl(g, s);
        self.leader_kind_aura(p, ctrl, self.kind(g, s) & 0x1F, n)
    }

    /// 0x267A00: stat modifier of `u` at (c, r): card 0x2A3's leader copy,
    /// continuous auras (types 0x12/0x13/0x22), terrain ±500, leader aura
    /// 0x273420.
    pub fn modifier(&self, u: &Unit, c: i32, r: i32, atk: bool) -> R<i32> {
        let ctrl = u.ctrl();
        let mut m = 0;
        if u.card == 0x2A3 {
            let own = if atk {
                i32::from(self.record(u)?.attack & 0x1FFF) + i32::from(u.atk_mod)
            } else {
                i32::from(self.record(u)?.defense & 0x1FFF) + i32::from(u.def_mod)
            };
            m -= own.min(9999);
            // Defence: the own leader's stat; otherwise the other leader's.
            let g = if u.bit(3) { ctrl } else { i32::from(ctrl == 0) };
            m += if atk {
                self.atk(g, 0, true)?
            } else {
                self.def(g, 0, true)?
            };
        }
        // Entry 2 of face-up defence units in slots 1..5, own group first
        // (0x267B50/0x267C68): types 0x12 (+value), 0x13 (-value) and 0x22
        // (+value to DEF only) where 0x2505F0 accepts `u`; any other type
        // adds nothing.
        for g in [ctrl, i32::from(ctrl == 0)] {
            for s in 1..6 {
                let Some(v) = self.unit(g, s) else { continue };
                if !(v.bit(4) && v.bit(3) && v.bit(5)) {
                    continue;
                }
                let Some([word0, word1]) = v.entry(2) else {
                    continue;
                };
                let ty = word0 >> 8;
                if !matches!(ty, 0x12 | 0x13 | 0x22) || !aura_applies(u, v.ctrl(), word0 & 0xFF) {
                    continue;
                }
                let value = (word1 & 0xFFFF) as i32;
                match ty {
                    0x12 => m += value,
                    0x13 => m -= value,
                    _ if !atk => m += value,
                    _ => (),
                }
            }
        }
        match self.terrain_bonus_kind(self.terrain(c, r), u)? {
            2 => m -= 500,
            1 => m += 500,
            _ => (),
        }
        m += self.leader_aura(u, c, r)?;
        Ok(m)
    }
    /// 0x273420: leader abilities #10 (own: +min(100 · L, 500) for the
    /// leader's kind inside the window, ABILITY-9-10.md §3) and #14 (the
    /// other side's leader, `enemy` = (ctrl == 0) as the `movn` at
    /// 0x27348C computes it: −min(100 · L, 500) when the cell lies in its
    /// window (0x273370), it lists #14 (0x273070) and its race weakens the
    /// unit's kind (0x2735C0, [`crate::duel::queries::weakens`])).
    fn leader_aura(&self, u: &Unit, c: i32, r: i32) -> R<i32> {
        let ctrl = u.ctrl();
        let enemy = i32::from(ctrl == 0);
        let kind = i32::from(u.kind & 0x1F);
        let mut sum = 0;
        if self.leader_kind_aura(r << 8 | c, ctrl, kind, 10)? {
            sum += (100 * self.leader_level(ctrl)).min(500);
        }
        if self.leader_window(enemy, c, r)?
            && self.ability(14, enemy)
            && crate::duel::queries::weakens(self.leader_kind(enemy), u.kind & 0x1F)
        {
            sum -= (100 * self.leader_level(enemy)).min(500);
        }
        Ok(sum)
    }
    /// 0x2521F0: clamp to 0..=9999.
    fn clamp_stat(v: i32) -> i32 {
        if v < 0 {
            0
        } else if v >= 10000 {
            9999
        } else {
            v
        }
    }
    /// 0x252300: effective ATK of (g, s) at (c, r).
    pub fn eff_atk(&self, g: i32, s: i32, c: i32, r: i32) -> R<i32> {
        let Some(u) = self.unit(g, s) else {
            return Ok(Self::clamp_stat(-1));
        };
        let m = self.modifier(u, c, r, true)?;
        Ok(Self::clamp_stat(self.atk_of(u, false)? + m))
    }
    /// 0x252380: effective ATK of a unit record at (c, r).
    pub fn eff_atk_of(&self, u: &Unit, c: i32, r: i32) -> R<i32> {
        let a = self.atk_of(u, false)?;
        Ok(Self::clamp_stat(a + self.modifier(u, c, r, true)?))
    }
    /// 0x252460: effective DEF of (g, s) at (c, r).
    pub fn eff_def(&self, g: i32, s: i32, c: i32, r: i32) -> R<i32> {
        let Some(u) = self.unit(g, s) else {
            return Ok(Self::clamp_stat(-1));
        };
        let m = self.modifier(u, c, r, false)?;
        Ok(Self::clamp_stat(self.def_of(u, false)? + m))
    }
    /// 0x2524E0: effective DEF of a unit record at (c, r).
    pub fn eff_def_of(&self, u: &Unit, c: i32, r: i32) -> R<i32> {
        let d = self.def_of(u, false)?;
        Ok(Self::clamp_stat(d + self.modifier(u, c, r, false)?))
    }

    /// 0x251760: the unit would be destroyed on Crush terrain at (c, r).
    pub fn crushed(&self, u: &Unit, c: i32, r: i32) -> R<bool> {
        if u.class() != 0 || u.slot == 0 || u.kind & 0x1F == 0x14 {
            return Ok(false);
        }
        Ok(self.eff_atk_of(u, c, r)? >= 1500)
    }

    /// 0x247E10: movement word of `u` measured at (c, r): low nibble = steps,
    /// bit 0x10 = may enter terrain 8 (0 = cannot move).
    pub fn allowance(&self, u: &Unit, c: i32, r: i32) -> R<i32> {
        if u.slot == 0 {
            return Ok(if self.ability(4, i32::from(u.side)) {
                2
            } else {
                1
            });
        }
        let e1 = u.entry_type(1);
        if !u.bit(2) {
            let mut v = 1;
            if e1 == 8 || (9..=10).contains(&e1) {
                v |= 0x10;
            } else if e1 == 0x4F {
                return Ok(0);
            }
            return Ok(v);
        }
        let mut s2 = match self.terrain_bonus_kind(self.terrain(c, r), u)? {
            1 => 2,
            2 | 0 => 1,
            _ => 0,
        };
        if e1 == 6 {
            s2 = (u.entries[1].expect("entry 1")[1] & 0xFFFF) as i32;
        } else if (8..=10).contains(&e1) {
            s2 |= 0x10;
        } else if e1 == 0x4F {
            return Ok(0);
        }
        for s in 0..11 {
            for g in 0..2 {
                let Some(v) = self.unit(g, s) else { continue };
                if !(v.bit(4) && v.bit(5)) {
                    continue;
                }
                let Some([w, w1]) = v.entry(2) else { continue };
                if w >> 8 != 6 {
                    continue;
                }
                if w & 0x3F != 0xF || (u.kind & 0x1F == 0xF && u.kind >> 5 == 0) {
                    s2 = (s2 >> 4) << 4 | (w1 & 0xFFFF) as i32;
                }
            }
        }
        if self.continuous_nth(0x43, 0) >= 0 {
            s2 = (s2 >> 4) << 4 | 1;
        }
        if self.leader_kind_aura(r << 8 | c, i32::from(u.side), i32::from(u.kind & 0x1F), 12)? {
            s2 = (s2 >> 4) << 4 | 2;
        }
        Ok(s2)
    }

    // ---- far-trap queries ---------------------------------------------------

    /// 0x26C5C0(side, mode, ty): the oldest (lowest summon order) class-4
    /// card with entry-0 type `ty` in slots 6..=10 of group `side` (mode
    /// bit 0) and of the other group (mode bit 1); −1 if none or while a
    /// continuous type-0xF unit is out.
    pub fn far_trap(&self, side: i32, mode: i32, ty: u32) -> i32 {
        if self.continuous_count(0xF) > 0 {
            return -1;
        }
        let mut best = 0xFF;
        let mut found = 0xFF;
        let mut scan = |g: i32| {
            for s in 6..11 {
                let Some(u) = self.unit(g, s) else { continue };
                if u.entry(0).is_none() || u.class() != 4 || u.entry_type(0) != ty {
                    continue;
                }
                let o = i32::from(u.order);
                if o < best {
                    best = o;
                    found = g << 8 | s;
                }
            }
        };
        if mode & 1 != 0 {
            scan(side);
        }
        if mode >> 1 != 0 {
            scan(i32::from(side == 0));
        }
        if found == 0xFF {
            -1
        } else {
            found
        }
    }
    /// 0x26C4B0(side, ty): 0x26C5C0 with mode 3 for types 0x2B, 0x2E,
    /// 0x11, 0x2D, 0x4C, 0x50 and mode 2 (the other side only) for 0x31,
    /// 0x30, 4, 0xE, 0x4B, 0x4A, 0x23, 0x2C, 0x2A, 0x29; −1 otherwise.
    pub fn far_trap_for(&self, side: i32, ty: u32) -> i32 {
        match ty {
            0x2B | 0x2E | 0x11 | 0x2D | 0x4C | 0x50 => self.far_trap(side, 3, ty),
            0x31 | 0x30 | 4 | 0xE | 0x4B | 0x4A | 0x23 | 0x2C | 0x2A | 0x29 => {
                self.far_trap(side, 2, ty)
            }
            _ => -1,
        }
    }
    /// 0x2738C0(g, s, c, r, n): (c, r) lies in the window of the unit's
    /// controller's leader (0x273370), that leader has ability `n`
    /// (0x273070) and its card's kind & 0x1F (0x1E1910) equals the unit's
    /// (0x1E1980 & 0x1F) — the #11 guard ([`Self::leader_kind_aura`] at a
    /// given cell).
    pub fn leader_guard(&self, g: i32, s: i32, c: i32, r: i32, n: u8) -> R<bool> {
        let ctrl = self.ctrl(g, s);
        self.leader_kind_aura(r << 8 | c, ctrl, self.kind(g, s) & 0x1F, n)
    }
    /// 0x268CA0(unit, viewer): 0 without a unit, for the viewer's own unit
    /// or a leader; 1 for a face-down one (+0x57 bit 2 clear); else the
    /// first continuous type-0x44 unit (0x268710 / 0x268590, bits 4, 3 and
    /// 5 set) of the unit's controller decides by its entry-2 filter: 0x1E
    /// → the unit is not a monster, 0x1D → it is one; 0.
    pub fn legend(&self, g: i32, s: i32, viewer: i32) -> bool {
        let Some(u) = self.unit(g, s) else {
            return false;
        };
        let ctrl = u.ctrl();
        if viewer == ctrl || u.slot == 0 {
            return false;
        }
        if !u.bit(2) {
            return true;
        }
        for i in 0..self.continuous_count(0x44) {
            let k = self.continuous_nth(0x44, i);
            let (kg, ks) = (k >> 8, k & 0xFF);
            if !(self.flag(kg, ks, 4) && self.flag(kg, ks, 3) && self.flag(kg, ks, 5)) {
                continue;
            }
            if self.ctrl(kg, ks) != ctrl {
                continue;
            }
            let Some([w0, _]) = self.unit(kg, ks).and_then(|v| v.entry(2)) else {
                continue;
            };
            match w0 & 0x3F {
                0x1E => return u.kind >> 5 != 0,
                0x1D => return u.kind >> 5 == 0,
                _ => (),
            }
        }
        false
    }
    /// 0x269020(g, s): a continuous type-0xB unit covers this monster.
    pub fn trap_immunity(&self, g: i32, s: i32) -> R<bool> {
        if s == 0 {
            return Ok(false);
        }
        let n = self.continuous_count(0xB);
        for i in 0..n {
            let k = self.continuous_nth(0xB, i);
            let (kg, ks) = (k >> 8, k & 0xFF);
            if !(self.flag(kg, ks, 4) && self.flag(kg, ks, 3) && self.flag(kg, ks, 5)) {
                continue;
            }
            let Some(v) = self.unit(kg, ks) else { continue };
            let Some([w, _]) = v.entry(2) else { continue };
            if w & 0x3F != 0 {
                continue;
            }
            let kind = self.kind(g, s);
            return Ok(kind & 0x1F == 0 && kind >> 5 == 0);
        }
        Ok(false)
    }
    /// 0x26CB20(g, s, which, see_all): whether an opposing Block Attack
    /// (which 0: 0x2C) or Gorgon's Eye / Shadow of Eyes (which 1: 0x4A, then
    /// 0x23) applies to (g, s), not cancelled by a Royal Decree (0x2E).
    /// Returns 0, 1 or 0x10001 (0x2C / 0x4A found).
    pub fn trap_hook(&self, g: i32, s: i32, which: i32, see_all: bool) -> R<i32> {
        let p = self.pos(g, s);
        let ctrl = self.ctrl(g, s);
        if s == 0 {
            return Ok(0);
        }
        // 0x26CB84: the #11 guard at the unit's cell; then 1 unless
        // 0x268CA0(unit, the other side) answers.
        if self.leader_guard(g, s, p & 0xFF, p >> 8, 0xB)? && !self.legend(g, s, i32::from(ctrl == 0)) {
            return Ok(1);
        }
        if self.trap_immunity(g, s)? {
            return Ok(0);
        }
        let other_g = i32::from(g == 0);
        let visible = |k: i32| see_all || self.flag(k >> 8, k & 0xFF, 4);
        if which == 0 {
            let t = self.far_trap_for(ctrl, 0x2C);
            if t < 0 || !visible(t) {
                return Ok(0);
            }
            return Ok(if self.far_trap_for(other_g, 0x2E) >= 0 {
                0
            } else {
                0x10001
            });
        }
        let t = self.far_trap_for(ctrl, 0x4A);
        if t >= 0 {
            if !visible(t) {
                return Ok(0);
            }
            return Ok(if self.far_trap_for(other_g, 0x2E) >= 0 {
                0
            } else {
                0x10001
            });
        }
        let t = self.far_trap_for(ctrl, 0x23);
        if t < 0 || !visible(t) {
            return Ok(0);
        }
        Ok(if self.far_trap_for(other_g, 0x2E) >= 0 {
            0
        } else {
            1
        })
    }

    /// 0x27FEF0(key, c, r): the controller's opponent has an effect-type-4
    /// trap (0x26C4B0), or an orthogonal neighbour holds a class-3 card
    /// other than 0x32D controlled by the same side.
    pub fn shielded(&self, k: i32, c: i32, r: i32) -> bool {
        let ctrl = self.ctrl(k >> 8, k & 0xFF);
        if self.far_trap_for(ctrl, 4) >= 0 {
            return true;
        }
        let c0 = (c - 1).max(0);
        let r0 = (r - 1).max(0);
        for cc in c0..=(c + 1).max(0) {
            for rr in r0..=(r + 1).max(0) {
                if (cc - c).abs() + (rr - r).abs() != 1 {
                    continue;
                }
                let k2 = self.at_key(cc, rr);
                if k2 < 0 {
                    continue;
                }
                let (g2, s2) = (k2 >> 8, k2 & 0xFF);
                if self.kind(g2, s2) >> 5 != 3 {
                    continue;
                }
                if self.card_id(g2, s2) == 0x32D {
                    continue;
                }
                if self.ctrl(g2, s2) == ctrl {
                    return true;
                }
            }
        }
        false
    }

    /// 0x27F170(g, s, a2, a3, mode, see_all) for modes 4, 5 and 6 (mode 0,
    /// which runs the trap simulator 0x24CC30, is ported by its only
    /// caller 0x2800C0): 5 or 2 when an opposing far trap applies, else 0.
    /// - mode 4: Gorgon's Eye (0x4A) → 2, Shadow of Eyes (0x23) → 5;
    /// - mode 5: Block Attack (0x2C) → 5;
    /// - mode 6: a continuous type-0xE unit, or a type-0xE trap the unit's
    ///   own side controls (visible or `see_all`) → 5.
    pub fn classify(&self, g: i32, s: i32, mode: i32, see_all: bool) -> R<i32> {
        match mode {
            6 => {
                if self.continuous_count(0xE) > 0 {
                    return Ok(5);
                }
                let ctrl = self.ctrl(g, s);
                let t = self.far_trap_for(i32::from(ctrl == 0), 0xE);
                if t < 0 {
                    return Ok(0);
                }
                if see_all || self.flag(t >> 8, t & 0xFF, 4) {
                    return Ok(5);
                }
                Ok(0)
            }
            5 => Ok(if self.trap_hook(g, s, 0, see_all)? & 0xFFFF == 1 {
                5
            } else {
                0
            }),
            4 => {
                let r = self.trap_hook(g, s, 1, see_all)?;
                if r & 0xFFFF != 1 {
                    return Ok(0);
                }
                Ok(if r >> 16 == 1 { 2 } else { 5 })
            }
            0 => unsupported("0x27F170 mode 0 belongs to 0x2800C0"),
            _ => Ok(0),
        }
    }

    // ---- battle margins -------------------------------------------------

    /// 0x27A130(u, v): the type-0x1B race bonus (entry 0 word1 & 0xFFFF)
    /// of `v` against `u`'s kind, else of `u` against `v`'s kind.
    pub fn race_bonus(u: &Unit, v: &Unit) -> i32 {
        let check = |a: &Unit, b: &Unit| -> Option<i32> {
            let [w, w1] = a.entry(0).unwrap_or([u32::MAX; 2]);
            (w >> 8 == 0x1B && (w & 0x3F) as u8 == b.kind & 0x1F && b.kind >> 5 == 0)
                .then_some((w1 & 0xFFFF) as i32)
        };
        check(v, u).or_else(|| check(u, v)).unwrap_or(0)
    }
    /// 0x279B90(att, def_key, c, r): attacker strength minus defender
    /// strength, both at (c, r).
    pub fn margin(&self, att: &Unit, def_key: i32, c: i32, r: i32) -> R<i32> {
        let a = if att.is_leader() || att.class() != 0 {
            0
        } else if i32::from(att.col) == c && i32::from(att.row) == r && att.bit(3) {
            self.eff_def_of(att, c, r)?
        } else {
            self.eff_atk_of(att, c, r)?
        };
        let (g, s) = (def_key >> 8, def_key & 0xFF);
        let def = self
            .unit(g, s)
            .ok_or(DuelError::InvalidCommand("0x279B90 without a defender"))?;
        let a = a + Self::race_bonus(att, def);
        let d = if def.is_leader() || def.class() != 0 {
            0
        } else if def.bit(5) {
            self.eff_def(g, s, c, r)?
        } else {
            self.eff_atk(g, s, c, r)?
        };
        Ok(a - (d + Self::race_bonus(def, att)))
    }
    /// 0x279B30(att_key, def_key, c, r).
    pub fn margin_k(&self, att: i32, def: i32, c: i32, r: i32) -> R<i32> {
        match self.unit_k(att) {
            Some(u) => self.margin(&u.clone(), def, c, r),
            None => Ok(0),
        }
    }
    /// 0x279D60(u, value, c, r): strength at (c, r) minus `value` (capped at
    /// 1500 on Crush).
    pub fn margin_vs(&self, u: &Unit, value: i32, c: i32, r: i32) -> R<i32> {
        let a = if u.is_leader() {
            0
        } else if i32::from(u.col) == c && i32::from(u.row) == r && u.bit(3) {
            self.eff_def_of(u, c, r)?
        } else {
            self.eff_atk_of(u, c, r)?
        };
        let v = if self.terrain(c, r) == 9 {
            value.min(1500)
        } else {
            value
        };
        Ok(a - v)
    }
    /// 0x279AF0(g, s, see_all): the AI may read the card (face-up or
    /// see_all).
    pub fn visible(&self, g: i32, s: i32, see_all: bool) -> bool {
        see_all || self.flag(g, s, 2)
    }
    /// 0x279E50(u, def_key, hidden, c, r, see_all).
    pub fn margin_seen(
        &self,
        u: &Unit,
        def_key: i32,
        hidden: i32,
        c: i32,
        r: i32,
        see_all: bool,
    ) -> R<i32> {
        if def_key == -1 || !self.visible(def_key >> 8, def_key & 0xFF, see_all) {
            return self.margin_vs(u, hidden, c, r);
        }
        self.margin(u, def_key, c, r)
    }

    // ---- unit lists -----------------------------------------------------

    /// 0x27DEE0(g, s, filter).
    pub fn filter_slot(s: i32, filter: i32) -> bool {
        match filter {
            0x34 => s != 0,
            0x1F => true,
            0x1E => (6..12).contains(&s),
            0x1D => (1..6).contains(&s),
            0x22 => s == 0,
            _ => s != 0,
        }
    }
    /// 0x27D9B0(side, filter, see_all): keys of units controlled by `side`
    /// (group `side` first, then the other group), in the filter's slot
    /// range, visible (0x279AF0) or passing 0x27DEE0; at most 22.
    pub fn list(&self, side: i32, filter: i32, see_all: bool) -> Vec<i32> {
        let (lo, hi) = match filter {
            0x34 => (1, 11),
            0x1F => (0, 11),
            0x22 => (0, 1),
            _ => (1, 11),
        };
        let mut out = Vec::new();
        for g in [side, i32::from(side == 0)] {
            for s in lo..hi {
                if out.len() >= 22 {
                    return out;
                }
                let Some(u) = self.unit(g, s) else { continue };
                if u.ctrl() != side {
                    continue;
                }
                if self.visible(g, s, see_all) || Self::filter_slot(s, filter) {
                    out.push(g << 8 | s);
                }
            }
        }
        out
    }
    /// 0x27DC00(side, filter, include_hidden, see_all): visible units
    /// passing 0x27DEE0, plus hidden ones when `include_hidden`.
    pub fn list_seen(
        &self,
        side: i32,
        filter: i32,
        include_hidden: bool,
        see_all: bool,
    ) -> Vec<i32> {
        let (lo, hi) = match filter {
            0x1D => (1, 6),
            0x34 => (1, 11),
            0x1F => (0, 11),
            0x22 => (0, 1),
            _ => (1, 11),
        };
        let mut out = Vec::new();
        for g in [side, i32::from(side == 0)] {
            for s in lo..hi {
                if out.len() >= 22 {
                    return out;
                }
                let Some(u) = self.unit(g, s) else { continue };
                if u.ctrl() != side {
                    continue;
                }
                if self.visible(g, s, see_all) {
                    if Self::filter_slot(s, filter) {
                        out.push(g << 8 | s);
                    }
                } else if include_hidden {
                    out.push(g << 8 | s);
                }
            }
        }
        out
    }

    /// 0x278600(c, r, ref_key, radius, hidden, see_all): Σ over the ref's
    /// opponents (0x27D9B0 filter 0x1D) at distance 0 < d < radius of
    /// max(0, −0x279E50(ref, enemy)) / d.
    pub fn danger(
        &self,
        c: i32,
        r: i32,
        ref_key: i32,
        radius: i32,
        hidden: i32,
        see_all: bool,
    ) -> R<i32> {
        let reference = self.unit_k(ref_key).cloned();
        let ctrl = reference.as_ref().map_or(-1, Unit::ctrl);
        let list = self.list(i32::from(ctrl == 0), 0x1D, see_all);
        let hidden = hidden & 0xFFFF;
        let radius = radius & 0xFF;
        let mut sum = 0;
        for k in list {
            if self.unit_k(k).is_none() {
                continue;
            }
            let p = self.pos(k >> 8, k & 0xFF);
            let (uc, ur) = (p & 0xFF, p >> 8);
            let d = (uc - c).abs() + (ur - r).abs();
            if radius < d || d <= 0 || d >= radius {
                continue;
            }
            let m = match &reference {
                Some(u) => self.margin_seen(u, k, hidden, uc, ur, see_all)?,
                None => 0,
            };
            let v = if m <= 0 { -m } else { 0 };
            sum += v / d;
        }
        Ok(sum)
    }

    // ---- step legality ----------------------------------------------------

    /// 0x27B200(u, fc, fr, tc, tr, occ): one move of at most two cells is
    /// passable (Labyrinth needs bit 0x10, Crush refused, an occupant only
    /// when it is `occ`); distance > 2 is illegal.
    #[allow(clippy::too_many_arguments)]
    pub fn step_ok(&self, u: &Unit, fc: i32, fr: i32, tc: i32, tr: i32, occ: i32) -> R<bool> {
        let allow = self.allowance(u, fc, fr)?;
        let d = (fc - tc).abs() + (fr - tr).abs();
        match d {
            0 => Ok(true),
            1 => {
                let t = self.terrain(tc, tr);
                if t == 8 && allow & 0x10 == 0 {
                    return Ok(false);
                }
                if t == 9 && self.crushed(u, tc, tr)? {
                    return Ok(false);
                }
                if self.at(tc, tr).is_none() {
                    return Ok(true);
                }
                Ok(occ & 0xFFFF == self.at_key(tc, tr))
            }
            2 => {
                let dc = (fc - tc).abs();
                if dc == 1 {
                    if self.step_ok(u, fc, fr, tc, fr, occ)?
                        && self.step_ok(u, tc, fr, tc, tr, occ)?
                    {
                        return Ok(true);
                    }
                    if !self.step_ok(u, fc, fr, fc, tr, occ)? {
                        return Ok(false);
                    }
                    return self.step_ok(u, fc, tr, tc, tr, occ);
                }
                if dc == 2 {
                    let mc = (fc + tc) >> 1;
                    if !self.step_ok(u, fc, fr, mc, tr, occ)? {
                        return Ok(false);
                    }
                    return self.step_ok(u, mc, fr, tc, tr, occ);
                }
                if dc == 0 {
                    let mr = (fr + tr) >> 1;
                    if !self.step_ok(u, fc, fr, tc, mr, occ)? {
                        return Ok(false);
                    }
                    return self.step_ok(u, fc, mr, tc, tr, occ);
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }
    /// 0x27B190(g, s, fc, fr, tc, tr, occ).
    #[allow(clippy::too_many_arguments)]
    pub fn step_ok_k(
        &self,
        g: i32,
        s: i32,
        fc: i32,
        fr: i32,
        tc: i32,
        tr: i32,
        occ: i32,
    ) -> R<bool> {
        match self.unit(g, s) {
            Some(u) => self.step_ok(&u.clone(), fc, fr, tc, tr, occ),
            None => Ok(false),
        }
    }
    /// 0x27B580(g, s, fc, fr, tc, tr, occ, own_ok): 0x27B600 for (g, s).
    #[allow(clippy::too_many_arguments)]
    pub fn move_ok_k(
        &self,
        g: i32,
        s: i32,
        fc: i32,
        fr: i32,
        tc: i32,
        tr: i32,
        occ: i32,
        own_ok: bool,
    ) -> R<bool> {
        match self.unit(g, s) {
            Some(u) => self.move_ok(&u.clone(), fc, fr, tc, tr, occ, own_ok),
            None => Ok(false),
        }
    }
    /// 0x27B600(u, fc, fr, tc, tr, occ, own_ok): as 0x27B200 for one step,
    /// but with `own_ok` a cell held by the own side is enterable; two
    /// cells use 0x27B200.
    #[allow(clippy::too_many_arguments)]
    pub fn move_ok(
        &self,
        u: &Unit,
        fc: i32,
        fr: i32,
        tc: i32,
        tr: i32,
        occ: i32,
        own_ok: bool,
    ) -> R<bool> {
        let allow = self.allowance(u, fc, fr)?;
        let d = (fc - tc).abs() + (fr - tr).abs();
        match d {
            0 => Ok(true),
            1 => {
                let t = self.terrain(tc, tr);
                if t == 8 && allow & 0x10 == 0 {
                    return Ok(false);
                }
                if t == 9 && self.crushed(u, tc, tr)? {
                    return Ok(false);
                }
                if self.at(tc, tr).is_none() {
                    return Ok(true);
                }
                let k = self.at_key(tc, tr);
                if occ & 0xFFFF == k {
                    return Ok(true);
                }
                if !own_ok {
                    return Ok(false);
                }
                Ok(self.ctrl(k >> 8, k & 0xFF) == u.ctrl())
            }
            2 => {
                let dc = (fc - tc).abs();
                if dc == 1 {
                    if self.step_ok(u, fc, fr, tc, fr, occ)?
                        && self.step_ok(u, tc, fr, tc, tr, occ)?
                    {
                        return Ok(true);
                    }
                    if !self.step_ok(u, fc, fr, fc, tr, occ)? {
                        return Ok(false);
                    }
                    return self.step_ok(u, fc, tr, tc, tr, occ);
                }
                if dc == 2 {
                    let mc = (fc + tc) >> 1;
                    if !self.step_ok(u, fc, fr, mc, tr, occ)? {
                        return Ok(false);
                    }
                    return self.step_ok(u, mc, fr, tc, tr, occ);
                }
                if dc == 0 {
                    let mr = (fr + tr) >> 1;
                    if !self.step_ok(u, fc, fr, tc, mr, occ)? {
                        return Ok(false);
                    }
                    return self.step_ok(u, fc, mr, tc, tr, occ);
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }
    /// 0x27C050(u, fc, fr, tc, tr, target, see_all, first, hidden): step
    /// legality for attack paths: 1 passable, 2 an enemy this unit would
    /// not beat, 0 illegal. Enemies it beats are passable; own non-leaders
    /// only after the first move; the target key always.
    #[allow(clippy::too_many_arguments)]
    pub fn attack_step(
        &self,
        u: &Unit,
        fc: i32,
        fr: i32,
        tc: i32,
        tr: i32,
        target: Option<u32>,
        see_all: bool,
        first: bool,
        hidden: i32,
    ) -> R<i32> {
        let allow = self.allowance(u, fc, fr)?;
        let d = (fc - tc).abs() + (fr - tr).abs();
        let rec = |a: i32, b: i32, c: i32, e: i32| {
            self.attack_step(u, a, b, c, e, target, see_all, first, hidden)
        };
        match d {
            0 => Ok(1),
            1 => {
                let t = self.terrain(tc, tr);
                if t == 8 && allow & 0x10 == 0 {
                    return Ok(0);
                }
                if t == 9 && self.crushed(u, tc, tr)? {
                    return Ok(0);
                }
                if self.at(tc, tr).is_none() {
                    return Ok(1);
                }
                let k = self.at_key(tc, tr);
                if target.is_some_and(|t| t == k as u32) {
                    return Ok(1);
                }
                let (g2, s2) = (k >> 8, k & 0xFF);
                if self.ctrl(g2, s2) != u.ctrl() {
                    let m = if self.visible(g2, s2, see_all) {
                        self.margin(u, k, tc, tr)?
                    } else {
                        self.margin_vs(u, hidden & 0xFFFF, tc, tr)?
                    };
                    return Ok(if m > 0 { 1 } else { 2 });
                }
                if s2 == 0 {
                    return Ok(0);
                }
                Ok(if first { 0 } else { 1 })
            }
            2 => {
                let dc = (fc - tc).abs();
                if dc == 1 {
                    if rec(fc, fr, tc, fr)? == 1 && rec(tc, fr, tc, tr)? == 1 {
                        return Ok(1);
                    }
                    if rec(fc, fr, fc, tr)? == 1 && rec(fc, tr, tc, tr)? == 1 {
                        return Ok(1);
                    }
                    return Ok(0);
                }
                if dc == 2 {
                    let mc = (fc + tc) >> 1;
                    if rec(fc, fr, mc, tr)? != 1 {
                        return Ok(0);
                    }
                    return rec(mc, fr, tc, tr);
                }
                if dc == 0 {
                    let mr = (fr + tr) >> 1;
                    if rec(fc, fr, tc, mr)? != 1 {
                        return Ok(0);
                    }
                    return rec(fc, mr, tc, tr);
                }
                Ok(0)
            }
            _ => Ok(0),
        }
    }

    // ---- actor tests --------------------------------------------------------

    /// 0x27D2F0(key): not acted, and some orthogonal neighbour is empty and
    /// enterable (Labyrinth needs bit 0x10, Crush refused when crushed).
    pub fn can_step(&self, k: i32) -> R<bool> {
        let (g, s) = (k >> 8, k & 0xFF);
        let p = self.pos(g, s);
        if p < 0 {
            return Ok(false);
        }
        if self.flag(g, s, 0) {
            return Ok(false);
        }
        let u = self.unit(g, s).expect("positioned unit").clone();
        let (c, r) = (p & 0xFF, p >> 8);
        let allow = self.allowance(&u, c, r)?;
        let wall_ok = allow & 0x10 != 0;
        for cc in (c - 1).max(0)..=(c + 1).min(6) {
            for rr in (r - 1).max(0)..=(r + 1).min(6) {
                if (cc - c).abs() + (rr - r).abs() != 1 {
                    continue;
                }
                if self.at(cc, rr).is_some() {
                    continue;
                }
                let t = self.terrain(cc, rr);
                if t == 8 && !wall_ok {
                    continue;
                }
                if t == 9 && self.crushed(&u, cc, rr)? {
                    continue;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// 0x27E010(key): false only for an un-acted class-3/4 card with bit 4.
    pub fn may_act(&self, k: i32) -> bool {
        let (g, s) = (k >> 8, k & 0xFF);
        if self.flag(g, s, 0) {
            return true;
        }
        let class = self.kind(g, s) >> 5;
        if class == 3 || class == 4 {
            return !self.flag(g, s, 4);
        }
        true
    }
    /// 0x27D500(g, s, see_all): some enemy (0x27D9B0 filter 0x1F) can reach
    /// this unit this turn and would tie or win (margin ≤ 0), is not bound,
    /// and the unit is not shielded (0x27FEF0).
    pub fn threatened(&self, g: i32, s: i32, see_all: bool) -> R<bool> {
        if self.unit(g, s).is_none() {
            return Ok(false);
        }
        let me = g << 8 | s;
        let list = self.list(i32::from(self.ctrl(g, s) == 0), 0x1F, see_all);
        let p = self.pos(g, s);
        let (mc, mr) = (p & 0xFF, p >> 8);
        for k in list {
            let (eg, es) = (k >> 8, k & 0xFF);
            let ep = self.pos(eg, es);
            let e = self.unit(eg, es).expect("listed unit").clone();
            let (ec, er) = (ep & 0xFF, ep >> 8);
            let allow = self.allowance(&e, ec, er)? & 0xF;
            let d = (ec - mc).abs() + (er - mr).abs();
            if (allow as u32) < d as u32 {
                continue;
            }
            if !self.step_ok(&e, ec, er, mc, mr, me)? {
                continue;
            }
            if self.margin_k(me, k, mc, mr)? > 0 {
                continue;
            }
            if self.spellbind(eg, es) != 0 || self.x4a(eg, es) != 0 {
                continue;
            }
            if self.shielded(me, mc, mr) {
                continue;
            }
            return Ok(true);
        }
        Ok(false)
    }
    /// 0x27D750(g, s, c, r, see_all): some enemy could reach (c, r) next
    /// turn and would tie or win against this unit there, unbound, and the
    /// cell is not shielded.
    pub fn cell_threatened(&self, g: i32, s: i32, c: i32, r: i32, see_all: bool) -> R<bool> {
        if self.unit(g, s).is_none() {
            return Ok(false);
        }
        let me = g << 8 | s;
        let list = self.list(i32::from(self.ctrl(g, s) == 0), 0x1F, see_all);
        for k in list {
            let (eg, es) = (k >> 8, k & 0xFF);
            let ep = self.pos(eg, es);
            let e = self.unit(eg, es).expect("listed unit").clone();
            let (ec, er) = (ep & 0xFF, ep >> 8);
            let allow = self.allowance(&e, ec, er)? & 0xF;
            let d = (ec - c).abs() + (er - r).abs();
            if (allow as u32) < d as u32 {
                continue;
            }
            if !self.step_ok(&e, ec, er, c, r, me)? {
                continue;
            }
            if self.margin_k(me, k, c, r)? > 0 {
                continue;
            }
            if self.spellbind(eg, es) != 0 || self.x4a(eg, es) != 0 {
                continue;
            }
            if self.shielded(me, c, r) {
                continue;
            }
            return Ok(true);
        }
        Ok(false)
    }

    // ---- hand prediction ----------------------------------------------------

    /// 0x271750(side): min(5, hand + deck).
    pub fn next_hand_size(&self, side: i32) -> i32 {
        (self.hand_count(side) + self.deck_count(side)).min(5)
    }
    /// 0x271660(side): the hand after the next summon draw: non-999 hand
    /// cards (& 0xFFF) in slot order, then deck cards while the count is
    /// below 0x271750.
    pub fn predicted_hand(&self, side: i32) -> Vec<i32> {
        let mut out: Vec<i32> = (0..5)
            .map(|i| self.hand(side, i) & 0xFFF)
            .filter(|&c| c != 999)
            .collect();
        let mut n = self.hand_count(side);
        let mut j = 0;
        while n < self.next_hand_size(side) {
            out.push(self.deck(side, j));
            n += 1;
            j += 1;
        }
        out
    }
    /// 0x1DE350(card): the card record; none for ids outside 0..0x356 and
    /// for 0x29F (Summoned Lord Exodia), which the lookup maps to 999.
    pub fn record_of(&self, card: i32) -> Option<&'a CardInfo> {
        if card == 0x29F {
            return None;
        }
        u16::try_from(card)
            .ok()
            .filter(|&c| c < 0x356)
            .and_then(|c| self.card(c))
    }
    /// 0x2713B0(card, side): summon cost = level, minus the leader-kind
    /// discount 0x273160 (ability #8); 0xFF without a record.
    pub fn cost(&self, card: i32, side: i32) -> R<i32> {
        let Some(info) = self.record_of(card) else {
            return Ok(0xFF);
        };
        let level = i32::from(info.level);
        if info.kind >> 5 != 0 {
            return Ok(level);
        }
        if i32::from(info.kind & 0x1F) == self.leader_kind(side) & 0x1F {
            return Ok(level - self.leader_discount(side));
        }
        Ok(level)
    }
    /// 0x273160(side): the ability-8 summon discount by the leader card's
    /// kind (0x1E1910 & 0x1F) and level L (0x1E1B70(side, 0, 0)): 0
    /// without the ability (0x273070); kind 0x14 → 1; 0xD / 4 → 2 at
    /// L ≥ 6 else 1; 0xE / 0xC / 6 / 0 → 2 at L ≥ 7 else 1; other kinds 0.
    pub fn leader_discount(&self, side: i32) -> i32 {
        if !self.ability(8, side) {
            return 0;
        }
        let level = self.leader_level(side);
        match self.leader_kind(side) & 0x1F {
            0x14 => 1,
            0xD | 4 => 1 + i32::from(level >= 6),
            0xE | 0xC | 6 | 0 => 1 + i32::from(level >= 7),
            _ => 0,
        }
    }
    /// 0x271440(side, card): stars ≥ cost (the summon planner keeps its
    /// own copy; this one serves the probe tests).
    #[cfg(test)]
    pub fn affordable(&self, side: i32, card: i32) -> R<bool> {
        Ok(self.stars(side) >= self.cost(card, side)?)
    }
}

/// Effect entries a unit record holds for `card` (0x1DDFE0): all five for
/// monsters, entry 0 for other cards.
pub(super) fn record_entries(card: &CardInfo) -> [Option<[u32; 2]>; 5] {
    let mut e = card.entries;
    if !card.is_monster() {
        for x in e.iter_mut().skip(1) {
            *x = None;
        }
    }
    e
}

/// 0x1DDFE0: an off-board unit record for `card` (hand evaluation).
pub(super) fn virtual_unit(card: &CardInfo) -> Unit {
    Unit {
        card: if card.id == 0x29F { 999 } else { card.id },
        kind: card.kind,
        attribute: card.attribute,
        col: 7,
        row: 7,
        side: 2,
        slot: 0xB,
        order: 0,
        discount: 0,
        spellbind: 0,
        x4a: 0,
        flags: 0,
        atk_mod: 0,
        def_mod: 0,
        entries: record_entries(card),
        id: None,
    }
}
