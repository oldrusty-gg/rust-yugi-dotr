//! Unit-record fields that duel effects rewrite, so that later readers see
//! the record and not the card table (FLIP-112-POWERUPS-789-791.md under
//! the project notes):
//!
//! - +0x3E kind byte and +0x3F attribute: flip types 0x36 / 0x37 (0x265250
//!   at 0x266270, event 0x42 → 0x26F220 → 0x1E19E0 / 0x1E1B10). Every
//!   kind/attribute reader of a field unit reads the record (0x1E1980,
//!   0x1E1AB0, 0x2505F0 +0x3E/+0x3F), so [`Duel::unit_card`] patches both.
//! - +0x08..+0x2F effect entries: category-1 power-ups (0x249D80 →
//!   0x249F3C, 0x1E1DF0) overwrite entries 0, 3 or 4 of the equipped
//!   monster's record.
//! - Riryoku (category 0, value 0, 0x249E40): the monster gains, and the
//!   opponent of the equipping side loses, 0x268A10(0x2C, opponent LP / 2).
//!
//! The records live beside the units (keyed by unit id) so that
//! [`DuelUnit`] keeps its shape; a unit created fresh (summon, fusion
//! result) has no entry and reads the card table, as 0x1E0870 copies it.

use super::*;
use std::borrow::Cow;

/// The rewritten fields of one unit record; `None` = as the card table.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitRecord {
    /// +0x3E (class << 5 | kind).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
    /// +0x3F.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<u8>,
    /// +0x08..+0x2F, all five entries once any was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<[Option<[u32; 2]>; 5]>,
}

/// 0x249F3C: the entries a category-1 power-up of `value` writes
/// (index, word0, word1). Other values write nothing (0x249F64).
pub(super) fn category1_entries(value: u16) -> &'static [(usize, u32, u32)] {
    match value {
        // Multiply 790: e4 := {0x145B, 0}, e3 := {0x385C, 0x1F0129}.
        1 => &[(4, 0x145B, 0), (3, 0x385C, 0x001F_0129)],
        // Sword of Dragon's Soul 791: e0 type 4, mode 2, filter 0 (Dragon).
        2 => &[(0, 0x480, 0)],
        // Enchanted Javelin 792: filter 7 (Fiend).
        3 => &[(0, 0x487, 0)],
        // Anti-Magic Fragrance 793: filter 1 (Spellcaster).
        4 => &[(0, 0x481, 0)],
        // Crush Card 794: e4 := {0x2FD, 0x10009}.
        5 => &[(4, 0x2FD, 0x0001_0009)],
        _ => &[],
    }
}

/// What a summon-list equip does besides the plain value (chain step 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EquipSpecial {
    /// Category 0 with a value: ATK/DEF + value.
    Plain,
    /// Category 0 with value 0 (Riryoku 789; 0x24AB20 → 0x219, handler
    /// 0x272560).
    Riryoku,
    /// Category 1: entry writes [`category1_entries`].
    Entries(u16),
}

impl Duel {
    /// The card as the unit's record holds it: the card table with the
    /// record's rewritten kind, attribute and effect entries.
    pub fn unit_card<'c, C: CardData + ?Sized>(
        &self,
        unit: &DuelUnit,
        cards: &'c C,
    ) -> Cow<'c, CardInfo> {
        let card = cards.card(unit.card).expect("validated card");
        match self.records.get(&unit.id.0) {
            None => Cow::Borrowed(card),
            Some(r) => {
                let mut c = card.clone();
                if let Some(k) = r.kind {
                    c.kind = k;
                }
                if let Some(a) = r.attribute {
                    c.attribute = a;
                }
                if let Some(e) = r.entries {
                    c.entries = e;
                    c.effects = e.map(|x| x.is_some());
                }
                Cow::Owned(c)
            }
        }
    }

    /// The record of a unit on the field, if it was rewritten.
    pub fn unit_record(&self, id: UnitId) -> Option<&UnitRecord> {
        self.records.get(&id.0)
    }

    fn record_mut(&mut self, id: UnitId) -> &mut UnitRecord {
        self.records.entry(id.0).or_default()
    }

    /// Writes category-1 entries over `base` (the record's current
    /// entries). Returns `None` when the value writes nothing.
    pub(super) fn engrave(
        base: [Option<[u32; 2]>; 5],
        value: u16,
    ) -> Option<[Option<[u32; 2]>; 5]> {
        let writes = category1_entries(value);
        if writes.is_empty() {
            return None;
        }
        let mut e = base;
        for &(k, w0, w1) in writes {
            e[k] = Some([w0, w1]);
        }
        Some(e)
    }

    /// Stores a rewritten entry set on a unit's record.
    pub(super) fn set_record_entries(&mut self, id: UnitId, entries: [Option<[u32; 2]>; 5]) {
        self.record_mut(id).entries = Some(entries);
    }

    /// Category-1 equip on a field unit (event 0x24 → 0x249D80 with a3 = 0).
    pub(super) fn engrave_unit(&mut self, id: UnitId, value: u16, cards: &impl CardData) {
        let Some(u) = self.unit(id) else {
            return;
        };
        let base = self.unit_card(u, cards).entries;
        if let Some(e) = Self::engrave(base, value) {
            self.set_record_entries(id, e);
        }
    }

    /// 0x24A750(side, 0x24A7C0()): half the LP of `side`'s opponent through
    /// the 0x2C loss modifiers. The type-0x34 sum 0x24A7C0 is 0 in every
    /// state the continuous-effect guard admits (Maha Vailo's source is
    /// refused there).
    pub(super) fn riryoku_amount(&self, side: u8, cards: &impl CardData) -> u16 {
        let lp = self.sides[usize::from(self.opp(side))].lp;
        // 0x24A750 adds the type-0x34 sum of 0x24A7C0 (Maha Vailo).
        let amount = i32::from(self.lp_modifier(0x2C, lp >> 1, cards)) + self.equip_modifier(cards);
        amount.clamp(0, i32::from(u16::MAX)) as u16
    }

    /// Flip types 0x36 (kind, +0x3E) and 0x37 (attribute, +0x3F),
    /// 0x266270–0x266324: filter 0x1A = the battle opponent (role 1 only,
    /// non-leader); any other filter = 0x24E360(0, 0, controller, mode,
    /// filter, 0xE), slots 1..=10 of both groups, whole board; the flipping
    /// unit is always removed (0x24E0F0). An empty mask writes no event.
    /// Event {0x42, type, mask, word1 & 0xFFFF}: 0x26F220 stores the low
    /// byte of the value into each target's record (0x1E19E0 / 0x1E1B10);
    /// no trap, immunity or face check [F; emu A/B/C].
    pub(super) fn flip_retype(
        &mut self,
        id: UnitId,
        opponent: Option<UnitId>,
        word0: u32,
        word1: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Flip> {
        let unit = self.unit(id).expect("flipping unit").clone();
        let (ty, low) = (word0 >> 8, word0 & 0xFF);
        let (mode, filter) = (low >> 6, low & 0x3F);
        let value = (word1 & 0xFF) as u8;
        let targets: Vec<UnitId> = if filter == 0x1A {
            opponent.filter(|&o| o != id).into_iter().collect()
        } else {
            let ctrl = unit.controller();
            let mut matched = Vec::new();
            for u in &self.units {
                if !(1..=10).contains(&u.slot) || u.id == id {
                    continue;
                }
                let side_ok = match mode {
                    1 => u.controller() == ctrl,
                    2 => self.foe(u.controller(), ctrl),
                    3 => true,
                    _ => false,
                };
                if side_ok && select_filter(filter, &self.unit_card(u, cards))? {
                    matched.push((u.side, u.slot, u.id));
                }
            }
            matched.sort_unstable();
            matched.into_iter().map(|(_, _, t)| t).collect()
        };
        if targets.is_empty() {
            return Ok(Flip::Continue);
        }
        events.push(DuelEvent::Activated {
            unit: id,
            card: unit.card,
            target: (filter == 0x1A).then(|| targets[0]),
        });
        for t in targets {
            let r = self.record_mut(t);
            if ty == 0x36 {
                r.kind = Some(value);
            } else {
                r.attribute = Some(value);
            }
        }
        Ok(Flip::Continue)
    }
}
