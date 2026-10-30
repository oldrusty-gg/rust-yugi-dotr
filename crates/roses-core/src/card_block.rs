//! The persistent per-side card block (private research
//! `claude-end/DUEL-END.md` §2–3, functions 0x2147F0–0x2160B0).
//!
//! The original keeps one 0xFFA0-byte block per side at
//! `BLK(s) = 0x2F2C10 + (s == byte[0x312B50] ? 0 : 0xFFA0)`; the block of
//! the pad-controlled side is what the memory card saves (0x213D70(0)). It
//! holds a 0x4C-byte record per card id (`CR(s, c) = BLK(s) + c·0x4C`, 854
//! records = 0xFD88 bytes) and a tail: the 5-entry recent list (u16 at
//! +0xFD88), the 16 alternate-art counters (bytes at +0xFE90), the owner key
//! (u32 at +0xFF98); the treasure bits at +0xFEA0 and the other tail bytes are
//! kept raw and not interpreted here.
//!
//! Record layout [F]: nine 8-byte instance slots (+0x00..+0x48; word 0 =
//! owner key with bit 31 kept, word 1 = 30-bit random id with bits 30–31
//! kept; a slot is free when word 0 & 0x7FFFFFFF == 0), u16 +0x48 = rank
//! (bits 0–13) | owned/seen (bit 14), u16 +0x4A = experience.
//!
//! Every accessor takes the card id the game passes: 0x29F is mapped to 999
//! and ids >= 854 are ignored (0x2152A0, 0x2154C0, 0x215570, 0x2157D0,
//! 0x215F90, 0x215D10 all start with that check). Provenance: [F] read from
//! the code, [emu] checked by running the game's code in the EE interpreter
//! (`claude-end/run1.py`, `run2.py`, `run4.py`), [H] hypothesis.

use crate::duel::{next_rand31, recompute_rank, ExperienceChange, RANKED_CARDS, RANK_COUNT};
use crate::CARD_COUNT;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Records per block (0x356, the bound of every card loop).
pub const BLOCK_CARDS: usize = CARD_COUNT as usize;
/// Bytes per card record.
pub const RECORD_BYTES: usize = 0x4C;
/// Instance slots per card record.
pub const INSTANCE_SLOTS: usize = 9;
/// Entries of the recent list at +0xFD88.
pub const RECENT_LEN: usize = 5;
/// Alternate-art counters at +0xFE90 (one per table entry, 0x215D10).
pub const ART_COUNTERS: usize = 16;
/// Bytes of one block in memory.
pub const BLOCK_BYTES: usize = 0xFFA0;
/// "No card" in the recent list (the same 999 the graveyard uses).
pub const EMPTY_CARD: u16 = 999;
/// Rank bits of the +0x48 word.
pub const RANK_MASK: u16 = 0x3FFF;
/// Owned/seen bit of the +0x48 word (byte +0x49 bit 6, set by 0x2157D0).
pub const OWNED_BIT: u16 = 0x4000;
/// Bits of the instance id that 0x2157D0 rolls (`rand() & 0x3FFFFFFF`).
pub const INSTANCE_ID_MASK: u32 = 0x3FFF_FFFF;
/// Bits of the instance owner word that hold the key.
pub const OWNER_KEY_MASK: u32 = 0x7FFF_FFFF;

const RECORDS_BYTES: usize = BLOCK_CARDS * RECORD_BYTES; // 0xFD88
const RECENT_OFFSET: usize = 0xFD88;
const ART_OFFSET: usize = 0xFE90;
const OWNER_KEY_OFFSET: usize = 0xFF98;
/// Raw tail bytes not interpreted: after the recent list up to the art
/// counters, after the art counters up to the owner key, after the key.
const RAW_RANGES: [(usize, usize); 3] = [
    (RECENT_OFFSET + 2 * RECENT_LEN, ART_OFFSET),
    (ART_OFFSET + ART_COUNTERS, OWNER_KEY_OFFSET),
    (OWNER_KEY_OFFSET + 4, BLOCK_BYTES),
];
const RAW_TAIL_BYTES: usize = (ART_OFFSET - RECENT_OFFSET - 2 * RECENT_LEN)
    + (OWNER_KEY_OFFSET - ART_OFFSET - ART_COUNTERS)
    + (BLOCK_BYTES - OWNER_KEY_OFFSET - 4);

/// The map editor's own maps (mode 0xD): five 7×7 terrain grids at
/// +0xFEA3 + 49·map, cell (x, y) at y·7 + x. The editor saves into the pad
/// side's block (0x213D70(0) + 0xFEA3, 0x1CDA64); Custom Duel's Edit Map
/// loads one into the board (0x1C7988) and the duel's map load 0x1DF0F0
/// reads maps 0x2E..0x32 from side 0's block and 0x33..0x37 from side 1's
/// [F]. 0x213A20 (boot) fills them with terrain 7.
pub const OWN_MAP_COUNT: usize = 5;
pub const OWN_MAP_CELLS: usize = 49;
pub const OWN_MAP_OFFSET: usize = 0xFEA3;
pub const NEW_OWN_MAP_TERRAIN: u8 = 7;

/// Index of own map `map`'s first cell inside `CardBlock::raw_tail` (the
/// maps lie in the second raw range, +0xFEA0..+0xFF98).
fn own_map_tail_index(map: usize) -> Option<usize> {
    if map >= OWN_MAP_COUNT {
        return None;
    }
    let (from, _) = RAW_RANGES[1];
    let first = RAW_RANGES[0].1 - RAW_RANGES[0].0;
    Some(first + OWN_MAP_OFFSET - from + OWN_MAP_CELLS * map)
}

/// Maps the id the game passes to a record index: 0x29F becomes 999 (out of
/// range), ids >= 854 are rejected.
fn record_index(card: u16) -> Option<usize> {
    let card = if card == crate::duel::REJECTED_DECK_CARD {
        EMPTY_CARD
    } else {
        card
    };
    (usize::from(card) < BLOCK_CARDS).then_some(usize::from(card))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardBlockError {
    /// 0x2152A0 op 3 with v = 0 divides by zero on the EE; its result is
    /// not defined by the code, so the model refuses it.
    DivisionByZero,
    /// A byte image with the wrong length.
    BlockLength(usize),
    /// A deserialised block with the wrong number of records.
    RecordCount(usize),
}
impl fmt::Display for CardBlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "card block: {self:?}")
    }
}
impl std::error::Error for CardBlockError {}

/// One 8-byte instance slot of a card record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardInstance {
    /// Word 0: the owner key (bits 0–30); bit 31 is kept as found.
    pub owner: u32,
    /// Word 1: the 30-bit random id; bits 30–31 are kept as found (the
    /// capture had 0xC0000000 in free slots of unowned cards [emu]; who sets
    /// them is untraced [H]).
    pub id: u32,
}
impl CardInstance {
    /// 0x2147F0 / 0x2157D0: a slot is free when word 0 & 0x7FFFFFFF == 0.
    pub fn is_free(self) -> bool {
        self.owner & OWNER_KEY_MASK == 0
    }
    /// The 30-bit random id 0x2147F0 compares.
    pub fn random_id(self) -> u32 {
        self.id & INSTANCE_ID_MASK
    }
}

/// One 0x4C-byte card record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardRecord {
    pub instances: [CardInstance; INSTANCE_SLOTS],
    /// u16 +0x48: rank (bits 0–13) | owned/seen (bit 14); bit 15 kept raw.
    pub rank_word: u16,
    /// u16 +0x4A.
    pub experience: u16,
}
impl CardRecord {
    /// The stored rank field (0x2154C0 reads it for ranked ids only).
    pub fn stored_rank(&self) -> u16 {
        self.rank_word & RANK_MASK
    }
    /// Bit 14 of +0x48, set by the grant.
    pub fn owned(&self) -> bool {
        self.rank_word & OWNED_BIT != 0
    }
    /// Occupied instance slots.
    pub fn copies(&self) -> usize {
        self.instances.iter().filter(|i| !i.is_free()).count()
    }
}

/// 0x2152A0's operation code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceOp {
    Add,
    Sub,
    Mul,
    Div,
    Set,
}

/// One alternate-art table entry (`roses_assets::AlternateArtRecord` with
/// the index already resolved): the card, the number of alternate pictures
/// and their picture record ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlternateArt {
    pub card: u16,
    pub variants: u8,
    pub pictures: [u16; 2],
}

/// A rank change found by the result state 0x5E (one PROMOTION/DEMOTION
/// banner each).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankChange {
    pub card: u16,
    pub old: u16,
    pub new: u16,
}
impl RankChange {
    pub fn demotion(&self) -> bool {
        self.new < self.old
    }
    /// The u16 the state writes to the list at 0x1ED0000: card | 0x4000 for
    /// a demotion | 0x8000 for side 1.
    pub fn entry_code(&self, side: u8) -> u16 {
        self.card | if self.demotion() { 0x4000 } else { 0 } | if side == 1 { 0x8000 } else { 0 }
    }
}

/// One side's card block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardBlock {
    /// 854 records, by card id.
    pub records: Vec<CardRecord>,
    /// u16[5] at +0xFD88, most recent first, 999 = empty (0x215F90).
    pub recent: [u16; RECENT_LEN],
    /// Bytes at +0xFE90, one per alternate-art entry (0x215D10).
    pub art_counters: [u8; ART_COUNTERS],
    /// u32 at +0xFF98: the key 0x2157D0 writes into a granted instance when
    /// the caller passes 0.
    pub owner_key: u32,
    /// The 0x1FA tail bytes not interpreted (+0xFD92..+0xFE90,
    /// +0xFEA0..+0xFF98, +0xFF9C..+0xFFA0), kept so images round-trip.
    pub raw_tail: Vec<u8>,
}

impl CardBlock {
    /// An all-zero block with an empty recent list and the given key. Not
    /// what a new game writes (untraced [H]); the capture's free slots
    /// carried word 1 = 0xC0000000, which this does not reproduce.
    pub fn empty(owner_key: u32) -> Self {
        Self {
            records: vec![CardRecord::default(); BLOCK_CARDS],
            recent: [EMPTY_CARD; RECENT_LEN],
            art_counters: [0; ART_COUNTERS],
            owner_key,
            raw_tail: vec![0; RAW_TAIL_BYTES],
        }
    }

    /// Decode the in-memory image of one block (0xFFA0 bytes at BLK(s)).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CardBlockError> {
        if bytes.len() != BLOCK_BYTES {
            return Err(CardBlockError::BlockLength(bytes.len()));
        }
        let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let u32_at =
            |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("fixed layout"));
        let records = (0..BLOCK_CARDS)
            .map(|c| {
                let base = c * RECORD_BYTES;
                let mut instances = [CardInstance::default(); INSTANCE_SLOTS];
                for (k, slot) in instances.iter_mut().enumerate() {
                    slot.owner = u32_at(base + 8 * k);
                    slot.id = u32_at(base + 8 * k + 4);
                }
                CardRecord {
                    instances,
                    rank_word: u16_at(base + 0x48),
                    experience: u16_at(base + 0x4A),
                }
            })
            .collect();
        let mut recent = [EMPTY_CARD; RECENT_LEN];
        for (k, r) in recent.iter_mut().enumerate() {
            *r = u16_at(RECENT_OFFSET + 2 * k);
        }
        let mut art_counters = [0; ART_COUNTERS];
        art_counters.copy_from_slice(&bytes[ART_OFFSET..ART_OFFSET + ART_COUNTERS]);
        let mut raw_tail = Vec::with_capacity(RAW_TAIL_BYTES);
        for (from, to) in RAW_RANGES {
            raw_tail.extend_from_slice(&bytes[from..to]);
        }
        Ok(Self {
            records,
            recent,
            art_counters,
            owner_key: u32_at(OWNER_KEY_OFFSET),
            raw_tail,
        })
    }

    /// The in-memory image (inverse of [`CardBlock::from_bytes`]).
    pub fn to_bytes(&self) -> Result<Vec<u8>, CardBlockError> {
        if self.records.len() != BLOCK_CARDS {
            return Err(CardBlockError::RecordCount(self.records.len()));
        }
        if self.raw_tail.len() != RAW_TAIL_BYTES {
            return Err(CardBlockError::BlockLength(self.raw_tail.len()));
        }
        let mut out = vec![0u8; BLOCK_BYTES];
        for (c, record) in self.records.iter().enumerate() {
            let base = c * RECORD_BYTES;
            for (k, slot) in record.instances.iter().enumerate() {
                out[base + 8 * k..base + 8 * k + 4].copy_from_slice(&slot.owner.to_le_bytes());
                out[base + 8 * k + 4..base + 8 * k + 8].copy_from_slice(&slot.id.to_le_bytes());
            }
            out[base + 0x48..base + 0x4A].copy_from_slice(&record.rank_word.to_le_bytes());
            out[base + 0x4A..base + 0x4C].copy_from_slice(&record.experience.to_le_bytes());
        }
        for (k, r) in self.recent.iter().enumerate() {
            out[RECENT_OFFSET + 2 * k..RECENT_OFFSET + 2 * k + 2].copy_from_slice(&r.to_le_bytes());
        }
        out[ART_OFFSET..ART_OFFSET + ART_COUNTERS].copy_from_slice(&self.art_counters);
        out[OWNER_KEY_OFFSET..OWNER_KEY_OFFSET + 4].copy_from_slice(&self.owner_key.to_le_bytes());
        let mut cursor = 0;
        for (from, to) in RAW_RANGES {
            out[from..to].copy_from_slice(&self.raw_tail[cursor..cursor + (to - from)]);
            cursor += to - from;
        }
        debug_assert_eq!(RECORDS_BYTES, RECENT_OFFSET);
        Ok(out)
    }

    /// Own map `map` (0..5) of the map editor: 49 terrain codes, cell
    /// (x, y) at y·7 + x (`None` past the fifth map).
    pub fn own_map(&self, map: usize) -> Option<[u8; OWN_MAP_CELLS]> {
        let at = own_map_tail_index(map)?;
        self.raw_tail.get(at..at + OWN_MAP_CELLS)?.try_into().ok()
    }

    /// Write own map `map` (the editor's save, 0x1CDA64..0x1CDAE4).
    pub fn set_own_map(&mut self, map: usize, cells: &[u8; OWN_MAP_CELLS]) -> bool {
        let Some(at) = own_map_tail_index(map) else { return false };
        match self.raw_tail.get_mut(at..at + OWN_MAP_CELLS) {
            Some(slot) => {
                slot.copy_from_slice(cells);
                true
            }
            None => false,
        }
    }

    /// 0x213A20 (boot): every own map filled with terrain 7.
    pub fn reset_own_maps(&mut self) {
        for map in 0..OWN_MAP_COUNT {
            self.set_own_map(map, &[NEW_OWN_MAP_TERRAIN; OWN_MAP_CELLS]);
        }
    }

    /// The record of a card the game would address (`None` for 0x29F and
    /// ids >= 854).
    pub fn record(&self, card: u16) -> Option<&CardRecord> {
        self.records.get(record_index(card)?)
    }
    fn record_mut(&mut self, card: u16) -> Option<&mut CardRecord> {
        let index = record_index(card)?;
        self.records.get_mut(index)
    }

    /// Experience u16 at +0x4A (`None` for ids the game ignores).
    pub fn experience(&self, card: u16) -> Option<u16> {
        self.record(card).map(|r| r.experience)
    }

    /// 0x2152A0(side, card, op, v): the result of the 32-bit operation on
    /// the stored value is clamped to 0..=0xFFFF and stored. Returns whether
    /// the card was accepted. [F][emu]
    pub fn change_experience(
        &mut self,
        card: u16,
        op: ExperienceOp,
        value: i32,
    ) -> Result<bool, CardBlockError> {
        let Some(record) = self.record_mut(card) else {
            return Ok(false);
        };
        let stored = i32::from(record.experience);
        let result = match op {
            ExperienceOp::Add => stored.wrapping_add(value),
            ExperienceOp::Sub => stored.wrapping_sub(value),
            ExperienceOp::Mul => stored.wrapping_mul(value),
            ExperienceOp::Div => {
                if value == 0 {
                    return Err(CardBlockError::DivisionByZero);
                }
                stored.wrapping_div(value)
            }
            ExperienceOp::Set => value,
        };
        record.experience = result.clamp(0, i32::from(u16::MAX)) as u16;
        Ok(true)
    }

    /// Apply a recorded series of 0x2152A0 add calls
    /// (`Duel::final_experience`). Returns whether the card was accepted.
    pub fn apply_change(&mut self, card: u16, change: &ExperienceChange) -> bool {
        match self.record_mut(card) {
            Some(record) => {
                record.experience = change.apply(record.experience);
                true
            }
            None => false,
        }
    }

    /// 0x2154C0(side, card): the stored rank; `None` (−1) for 0x29F and ids
    /// >= 0x2AB.
    pub fn stored_rank(&self, card: u16) -> Option<u16> {
        if card >= RANKED_CARDS {
            return None;
        }
        self.record(card).map(CardRecord::stored_rank)
    }

    /// 0x215570(side, card): recompute the rank from the experience and
    /// store it in bits 0–13 (bits 14–15 kept); `None` for unranked ids.
    /// [F][emu run4]
    pub fn recompute_rank(&mut self, card: u16, thresholds: &[u16; RANK_COUNT]) -> Option<u16> {
        let record = self.record_mut(card)?;
        let new = recompute_rank(card, record.experience, record.rank_word, thresholds)?;
        record.rank_word = (record.rank_word & !RANK_MASK) | new;
        Some(new)
    }

    /// Result state 0x5E (0x1D8CB0) for one side's block: every card id
    /// 0..0x356 in order, old = stored rank, new = recomputed and stored;
    /// changed ranks of cards for which `resident(card)` holds (0x200270(1,
    /// c): the card graphic is loaded) are listed. Residency is not traced
    /// here; all 40 deck cards were resident in the capture [emu], so a
    /// caller without better information passes `|_| true`. [F][emu run4]
    pub fn recompute_all_ranks(
        &mut self,
        thresholds: &[u16; RANK_COUNT],
        resident: impl Fn(u16) -> bool,
    ) -> Vec<RankChange> {
        let mut out = Vec::new();
        for card in 0..BLOCK_CARDS as u16 {
            let old = self.stored_rank(card);
            let new = self.recompute_rank(card, thresholds);
            if let (Some(old), Some(new)) = (old, new) {
                if old != new && resident(card) {
                    out.push(RankChange { card, old, new });
                }
            }
        }
        out
    }

    /// 0x2147F0(side, key, id, card): whether an occupied instance slot of
    /// this card already carries the 30-bit id. Only the id is compared;
    /// the key is validated but not matched. [F]
    pub fn is_duplicate(&self, card: u16, id: u32) -> bool {
        self.record(card).is_some_and(|r| {
            r.instances
                .iter()
                .any(|slot| !slot.is_free() && slot.random_id() == id & INSTANCE_ID_MASK)
        })
    }

    /// 0x2157D0(side, key, card): grant one copy. `key` 0/None = the block's
    /// owner key (0 → nothing). The first free instance slot gets a 30-bit id
    /// from the shared LCG, re-rolled while [`CardBlock::is_duplicate`]
    /// reports it (bits 30–31 of the word are kept), then the key (bit 31
    /// kept); the record's owned bit is set and the card moves to the front
    /// of the recent list. Nine copies already → nothing, `false`.
    /// [F][emu run2: 820/809/406, ids = rand31 & 0x3FFFFFFF]
    pub fn grant(&mut self, card: u16, key: Option<u32>, rng: &mut u64) -> bool {
        let key = match key {
            Some(k) if k != 0 => k,
            _ => self.owner_key,
        };
        if key == 0 || record_index(card).is_none() {
            return false;
        }
        let Some(free) = self
            .record(card)
            .and_then(|r| r.instances.iter().position(|s| s.is_free()))
        else {
            return false;
        };
        loop {
            let id = next_rand31(rng) & INSTANCE_ID_MASK;
            let record = self.record_mut(card).expect("validated above");
            let slot = &mut record.instances[free];
            slot.id = (slot.id & !INSTANCE_ID_MASK) | id;
            if !self.is_duplicate(card, id) {
                break;
            }
        }
        let record = self.record_mut(card).expect("validated above");
        let slot = &mut record.instances[free];
        slot.owner = (slot.owner & !OWNER_KEY_MASK) | (key & OWNER_KEY_MASK);
        record.rank_word |= OWNED_BIT;
        self.touch_recent(card);
        true
    }

    /// 0x215F90(side, card): move the card to the front of the 5-entry
    /// recent list (entries after its old position, or the last one when it
    /// was absent, keep their place). Returns whether the card was accepted.
    /// [F][emu run2: 820, 809, 406 → [406, 809, 820, 999, 999]]
    pub fn touch_recent(&mut self, card: u16) -> bool {
        if record_index(card).is_none() {
            return false;
        }
        let at = self
            .recent
            .iter()
            .position(|&r| r == card)
            .unwrap_or(RECENT_LEN - 1);
        for k in (1..=at).rev() {
            self.recent[k] = self.recent[k - 1];
        }
        self.recent[0] = card;
        true
    }

    /// 0x215D10(side, card): for a card in the alternate-art table, advance
    /// its counter modulo `variants + 1`. Returns whether an entry matched.
    /// [F; what the counter shows is `art_picture`]
    pub fn cycle_art(&mut self, card: u16, table: &[AlternateArt]) -> bool {
        if record_index(card).is_none() {
            return false;
        }
        let Some(k) = table.iter().take(ART_COUNTERS).position(|e| e.card == card) else {
            return false;
        };
        let modulus = u16::from(table[k].variants) + 1;
        self.art_counters[k] = ((u16::from(self.art_counters[k]) + 1) % modulus) as u8;
        true
    }

    /// 0x215E60(side, card): the picture record to draw for the card: its
    /// own (= the card id) unless its art counter is nonzero, then the
    /// counter-th alternate of its table entry. [F]
    pub fn art_picture(&self, card: u16, table: &[AlternateArt]) -> u16 {
        let shown = if card == crate::duel::REJECTED_DECK_CARD {
            EMPTY_CARD
        } else {
            card
        };
        if record_index(card).is_none() {
            return shown;
        }
        match table.iter().take(ART_COUNTERS).position(|e| e.card == card) {
            Some(k) if self.art_counters[k] != 0 => {
                let counter = usize::from(self.art_counters[k]);
                // The raw u16 at entry + 2·counter: counter 1/2 → pictures[0/1].
                table[k].pictures.get(counter - 1).copied().unwrap_or(shown)
            }
            _ => shown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: [u16; RANK_COUNT] = [
        0, 500, 1000, 2000, 4000, 7000, 11000, 16000, 22000, 30000, 40000, 52000, 65535,
    ];

    #[test]
    fn experience_ops_clamp_like_0x2152a0() {
        let mut b = CardBlock::empty(1);
        assert!(b.change_experience(10, ExperienceOp::Set, 70000).unwrap());
        assert_eq!(b.experience(10), Some(0xFFFF));
        b.change_experience(10, ExperienceOp::Sub, 0x10000).unwrap();
        assert_eq!(b.experience(10), Some(0));
        b.change_experience(10, ExperienceOp::Add, 12).unwrap();
        b.change_experience(10, ExperienceOp::Mul, 3).unwrap();
        b.change_experience(10, ExperienceOp::Div, 4).unwrap();
        assert_eq!(b.experience(10), Some(9));
        assert_eq!(
            b.change_experience(10, ExperienceOp::Div, 0),
            Err(CardBlockError::DivisionByZero)
        );
        // 0x29F → 999 and ids >= 854 are ignored.
        assert!(!b.change_experience(0x29F, ExperienceOp::Add, 5).unwrap());
        assert!(!b.change_experience(854, ExperienceOp::Add, 5).unwrap());
        assert_eq!(b.experience(0x29F), None);
    }

    #[test]
    fn ranks_recompute_and_store() {
        let mut b = CardBlock::empty(1);
        b.records[221].experience = 502;
        b.records[221].rank_word = OWNED_BIT;
        assert_eq!(b.stored_rank(221), Some(0));
        assert_eq!(b.recompute_rank(221, &T), Some(1));
        assert_eq!(b.records[221].rank_word, OWNED_BIT | 1);
        // Never back to NCO once ranked.
        b.records[221].experience = 3;
        assert_eq!(b.recompute_rank(221, &T), Some(1));
        // Unranked ids.
        assert_eq!(b.stored_rank(0x2AB), None);
        assert_eq!(b.recompute_rank(0x2AB, &T), None);
        assert_eq!(b.recompute_rank(0x29F, &T), None);
    }

    #[test]
    fn rank_list_matches_run4() {
        // run4.py: 221 (0x4000, 498) +4 → PROMOTION 0x00DD; 392 (0x4002,
        // 1005) −16 → DEMOTION 0x4188 (list read before the reel buffer
        // overwrote it: CAA at state 0x5F).
        let mut b = CardBlock::empty(1);
        b.records[221] = CardRecord {
            rank_word: 0x4000,
            experience: 498,
            ..Default::default()
        };
        b.records[392] = CardRecord {
            rank_word: 0x4002,
            experience: 1005,
            ..Default::default()
        };
        let mut win = b.clone();
        win.change_experience(221, ExperienceOp::Add, 4).unwrap();
        win.change_experience(392, ExperienceOp::Add, 24).unwrap();
        let list = win.recompute_all_ranks(&T, |_| true);
        assert_eq!(
            list,
            vec![RankChange {
                card: 221,
                old: 0,
                new: 1
            }]
        );
        assert_eq!(list[0].entry_code(0), 0x00DD);
        assert_eq!(
            (win.records[221].rank_word, win.records[392].rank_word),
            (0x4001, 0x4002)
        );
        let mut loss = b;
        loss.change_experience(392, ExperienceOp::Add, -16).unwrap();
        let list = loss.recompute_all_ranks(&T, |_| true);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].entry_code(0), 0x4188);
        assert_eq!(list[0].entry_code(1), 0xC188);
        assert_eq!(loss.records[392].rank_word, 0x4001);
        // Non-resident cards are stored but not listed.
        let mut quiet = loss.clone();
        quiet.records[392].experience = 2500;
        assert!(quiet.recompute_all_ranks(&T, |_| false).is_empty());
        assert_eq!(quiet.stored_rank(392), Some(3));
    }

    #[test]
    fn grant_fills_slots_rerolls_duplicates_and_touches_recent() {
        let mut b = CardBlock::empty(0x6D92_1E28);
        let mut rng = 0x1234u64;
        let mut probe = rng;
        let first = next_rand31(&mut probe) & INSTANCE_ID_MASK;
        assert!(b.grant(820, None, &mut rng));
        let r = &b.records[820];
        assert_eq!(r.instances[0].owner, 0x6D92_1E28);
        assert_eq!(r.instances[0].random_id(), first);
        assert!(r.owned());
        assert_eq!(b.recent, [820, 999, 999, 999, 999]);
        // A duplicate id is re-rolled: pre-seed slot 1 with the next id.
        let mut probe2 = rng;
        let next = next_rand31(&mut probe2) & INSTANCE_ID_MASK;
        let after_next = next_rand31(&mut probe2) & INSTANCE_ID_MASK;
        b.records[820].instances[1] = CardInstance {
            owner: 1,
            id: 0xC000_0000 | next,
        };
        assert!(b.is_duplicate(820, next));
        assert!(b.grant(820, Some(0x8000_0007), &mut rng));
        assert_eq!(b.records[820].instances[2].random_id(), after_next);
        assert_eq!(b.records[820].instances[2].owner, 7);
        assert_eq!(rng, probe2);
        // Nine copies → refused, nothing consumed.
        for slot in b.records[820].instances.iter_mut() {
            slot.owner |= 1;
        }
        let before = rng;
        assert!(!b.grant(820, None, &mut rng));
        assert_eq!(rng, before);
        // Key 0 with no owner key → nothing; 0x29F → nothing.
        let mut zero = CardBlock::empty(0);
        assert!(!zero.grant(5, None, &mut rng));
        assert!(!b.grant(0x29F, None, &mut rng));
        assert_eq!(rng, before);
    }

    #[test]
    fn recent_list_is_move_to_front() {
        let mut b = CardBlock::empty(1);
        for c in [820, 809, 406] {
            b.touch_recent(c);
        }
        assert_eq!(b.recent, [406, 809, 820, 999, 999]);
        b.touch_recent(820);
        assert_eq!(b.recent, [820, 406, 809, 999, 999]);
        for c in [1, 2, 3] {
            b.touch_recent(c);
        }
        assert_eq!(b.recent, [3, 2, 1, 820, 406]);
        b.touch_recent(406);
        assert_eq!(b.recent, [406, 3, 2, 1, 820]);
        assert!(!b.touch_recent(900));
    }

    #[test]
    fn art_counters_cycle() {
        let table = [
            AlternateArt {
                card: 81,
                variants: 2,
                pictures: [863, 864],
            },
            AlternateArt {
                card: 14,
                variants: 1,
                pictures: [855, 999],
            },
        ];
        let mut b = CardBlock::empty(1);
        assert_eq!(b.art_picture(81, &table), 81);
        assert!(b.cycle_art(81, &table));
        assert_eq!(b.art_picture(81, &table), 863);
        b.cycle_art(81, &table);
        assert_eq!(b.art_picture(81, &table), 864);
        b.cycle_art(81, &table);
        assert_eq!(b.art_picture(81, &table), 81);
        assert!(!b.cycle_art(15, &table));
        assert_eq!(b.art_picture(15, &table), 15);
        assert_eq!(b.art_counters[1], 0);
    }

    #[test]
    fn bytes_round_trip() {
        let mut b = CardBlock::empty(0xABCD_EF01);
        b.records[3].instances[4] = CardInstance {
            owner: 0x8000_0001,
            id: 0xC000_0002,
        };
        b.records[853].rank_word = 0x4005;
        b.records[853].experience = 777;
        b.recent = [1, 2, 3, 999, 999];
        b.art_counters[9] = 2;
        b.raw_tail[0] = 0x55;
        let bytes = b.to_bytes().unwrap();
        assert_eq!(bytes.len(), BLOCK_BYTES);
        assert_eq!(&bytes[0xFF98..0xFF9C], &[0x01, 0xEF, 0xCD, 0xAB]);
        assert_eq!(bytes[0xFD92], 0x55);
        assert_eq!(CardBlock::from_bytes(&bytes).unwrap(), b);
        assert_eq!(
            CardBlock::from_bytes(&bytes[1..]),
            Err(CardBlockError::BlockLength(BLOCK_BYTES - 1))
        );
        let json = serde_json::to_string(&b).unwrap();
        assert_eq!(serde_json::from_str::<CardBlock>(&json).unwrap(), b);
    }

    /// The captured blocks of state 767 (`claude-end/dump_fixture.py`):
    /// `ROSES_TEST_END_FIXTURE` = the directory with blk0.bin / blk1.bin.
    #[test]
    #[ignore = "requires the private savestate dump; set ROSES_TEST_END_FIXTURE"]
    fn private_capture_blocks_match_the_interpreter() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("ROSES_TEST_END_FIXTURE").expect("ROSES_TEST_END_FIXTURE"),
        );
        let raw0 = std::fs::read(dir.join("blk0.bin")).unwrap();
        let raw1 = std::fs::read(dir.join("blk1.bin")).unwrap();
        let mut b0 = CardBlock::from_bytes(&raw0).unwrap();
        let b1 = CardBlock::from_bytes(&raw1).unwrap();
        assert_eq!(b0.to_bytes().unwrap(), raw0);
        assert_eq!(b0.owner_key, 0x6D92_1E28);
        assert_eq!(b0.records[392].rank_word, 0x4001);
        assert_eq!(b0.experience(392), Some(500));
        assert_eq!(b0.experience(221), Some(2));
        assert_eq!(
            (b1.records[397].rank_word, b1.experience(397)),
            (0x4002, Some(1000))
        );
        assert_eq!(b0.recent, [999; RECENT_LEN]);
        // run2.py: after the duel-end award, 64 shuffle + 3 slip rand calls,
        // the prizes 820, 809, 406 are granted with key 0 → owner key.
        let mut rng = 0x1105_4A74_C1E1_7A22u64;
        for _ in 0..67 {
            next_rand31(&mut rng);
        }
        for card in [820, 809, 406] {
            assert!(b0.grant(card, Some(0), &mut rng));
        }
        let slot = |c: usize| b0.records[c].instances[0];
        assert_eq!((slot(820).owner, slot(820).id), (0x6D92_1E28, 0xE64E_5DF2));
        assert_eq!((slot(809).owner, slot(809).id), (0x6D92_1E28, 0xE405_1ADA));
        assert_eq!((slot(406).owner, slot(406).id), (0x6D92_1E28, 0xFA67_CCEA));
        assert!([820, 809, 406]
            .iter()
            .all(|&c| b0.records[c].rank_word == 0x4000));
        assert_eq!(b0.recent, [406, 809, 820, 999, 999]);
    }

    #[test]
    fn own_maps_lie_at_fea3_in_the_image() {
        let mut b = CardBlock::empty(1);
        b.reset_own_maps();
        let bytes = b.to_bytes().unwrap();
        assert!(bytes[0xFEA3..0xFF98].iter().all(|&t| t == 7));
        assert_eq!(bytes[0xFEA2], 0);
        let cells: [u8; OWN_MAP_CELLS] = std::array::from_fn(|i| (i % 10) as u8);
        assert!(b.set_own_map(4, &cells));
        assert!(!b.set_own_map(5, &cells));
        let bytes = b.to_bytes().unwrap();
        assert_eq!(&bytes[0xFEA3 + 4 * 49..0xFF98], &cells[..]);
        assert_eq!(bytes[0xFF98..0xFF9C], 1u32.to_le_bytes());
        let back = CardBlock::from_bytes(&bytes).unwrap();
        assert_eq!(back.own_map(4), Some(cells));
        assert_eq!(back.own_map(0), Some([7; OWN_MAP_CELLS]));
        assert_eq!(back.own_map(5), None);
    }
}