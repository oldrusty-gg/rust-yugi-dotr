//! The deck editor (mode 6) of PAL SLES_515.79 as a pure, frame-stepped
//! model (private research `modes/the project notes,
//! functions 0x1B9100 init, 0x1B94B0 update, 0x1BB0A0–0x1BB400 helpers,
//! 0x214560–0x216580 card-block operations, 0x251D50/0x251F40
//! reincarnation, 0x1BB110 passwords).
//!
//! The model owns the editor's own state (screen state, pane slide, the two
//! list buffers at W = 0x1ED0000, cursors, sort keys, password buffer,
//! reincarnation counters) and edits the persistent data it is given each
//! frame: the side's [`CardBlock`] (instance categories and leader flags,
//! the three deck records and the current-deck byte that live in its raw
//! tail, the owned/password bits and the recent list) and the campaign
//! save's duel counter `SAVE+0xCA`. Nothing is written on exit; every edit
//! is applied to the block immediately, as in the game.
//!
//! Provenance tags: [F] read from the code, [emu] the game's own code run
//! in the EE interpreter from the map savestate 622-z0
//! (`tools/emu_deck.py`, `tools/fixtures_deck.py`), [H] hypothesis.
//! Presentation (strings, layout, card images, animation slots, the
//! background scroll phases, the 3D viewer of mode 7) is not modelled
//! beyond the events the update emits.
//!
//! Untraced, kept explicit:
//! - The pad driver's auto-repeat counters (0x203380 kind 2 / 0x203490):
//!   [`PadState`] implements the formula read from 0x203380 (a held button
//!   fires when its counter is >= 0 and divisible by the rate; a restart
//!   sets the counter to minus the delay) with the counter incremented once
//!   per polled frame [H]; the per-frame increment and the value set on a
//!   new press are not traced.
//! - Whether a grant (0x2157D0) can land outside the trunk: the free slots'
//!   word-1 top bits were 0xC0000000 (trunk) in every observed slot [emu];
//!   the model grants into whatever bits the free slot carries, like the
//!   game.
//! - The "busy" flag 0x1FEC40 (graphics loading) is an input
//!   ([`EditorEnv::graphics_busy`]); the research ran it as 0.

use crate::CARD_COUNT;
use crate::card_block::{CardBlock, CardInstance, EMPTY_CARD, OWNED_BIT, OWNER_KEY_MASK};
use crate::duel::{RANKED_CARDS, REJECTED_DECK_CARD, next_rand31};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Cards per deck record (u16[40] at `BLK+0xFD94+d·0x54`).
pub const DECK_SIZE: usize = 40;
/// Deck records per block.
pub const DECKS_PER_BLOCK: u8 = 3;
/// Copies of one card a deck may hold (0x214C70).
pub const MAX_COPIES_IN_DECK: usize = 3;
/// Rows visible in each list.
pub const VISIBLE_ROWS: u16 = 6;
/// Entries of the deck list buffer: the leader row plus the 40 slots.
pub const DECK_LIST_LEN: usize = DECK_SIZE + 1;
/// Pane slide step per frame (0x1B96B4).
pub const SLIDE_STEP: i16 = 0x2D;
/// Campaign duels (`SAVE+0xCA`) needed to reincarnate (0x1B9CBC).
pub const REINCARNATION_DUELS: u8 = 5;
/// The card 0x251D50 returns when no candidate matches.
pub const REINCARNATION_FALLBACK: u16 = 820;
/// Frame of each reincarnation sub-step at which the result is granted.
pub const REINCARNATION_GRANT_FRAME: u8 = 0x7F;
/// Idle timer value set by any new press (0x1B9704).
pub const IDLE_FRAMES: u16 = 0x5A;
/// Sort keys at init (0x1B92EC): recent list, kind byte, card id.
pub const DEFAULT_SORT_KEYS: [u8; 3] = [0, 4, 0xA];
/// Catalogue page base limit (0x1B986C).
pub const CATALOGUE_MAX_BASE: u8 = 0x4A;
/// Bytes of one static card record at 0x2BE400.
pub const RECORD_BYTES: usize = 20;
/// Instance category meaning "in the trunk" (bits 30–31 of word 1).
pub const TRUNK_CATEGORY: u8 = 3;

/// ELF file offsets of the static data the editor reads
/// (EE address − 0xF9680).
pub mod elf {
    /// 854 × 20-byte card records (EE 0x2BE400).
    pub const CARD_TABLE: usize = 0x1C4D80;
    /// The 8-byte password key (EE 0x2BE2F9).
    pub const PASSWORD_KEY: usize = 0x1C4C79;
    /// Name-order tables, u16 per card, one per language (EE 0x2CBEC8).
    pub const NAME_ORDER: usize = 0x1D2848;
    /// Stride between the language tables.
    pub const NAME_ORDER_STRIDE: usize = 0x6AC;
    /// Languages with a table (0x1B9164; others fall back to 0).
    pub const LANGUAGES: u8 = 5;
}

/// Pad word bits as 0x203230 builds them (`(raw[2] << 8 | raw[3]) ^ 0xFFFF`).
pub mod pad {
    pub const L2: u16 = 0x1;
    pub const R2: u16 = 0x2;
    pub const L1: u16 = 0x4;
    pub const R1: u16 = 0x8;
    pub const TRIANGLE: u16 = 0x10;
    pub const CIRCLE: u16 = 0x20;
    pub const CROSS: u16 = 0x40;
    pub const SQUARE: u16 = 0x80;
    pub const SELECT: u16 = 0x100;
    pub const L3: u16 = 0x200;
    pub const R3: u16 = 0x400;
    pub const START: u16 = 0x800;
    pub const UP: u16 = 0x1000;
    pub const RIGHT: u16 = 0x2000;
    pub const DOWN: u16 = 0x4000;
    pub const LEFT: u16 = 0x8000;
}

/// Sound ids passed to 0x216EA0(0, id) [F][emu].
pub mod sound {
    pub const CURSOR: u8 = 3;
    pub const ACCEPT: u8 = 4;
    pub const LEAVE: u8 = 5;
    pub const REFUSED: u8 = 6;
    pub const GRANT: u8 = 7;
    pub const DECK_SWITCH: u8 = 9;
    pub const OPEN: u8 = 0x10;
    pub const CLOSE: u8 = 0x11;
    pub const PANE: u8 = 0x12;
    pub const SORT: u8 = 0x13;
}

/// Screen states (jump table 0x2BDD90).
pub mod state {
    pub const CATALOGUE: u8 = 0;
    pub const TRUNK_DETAIL: u8 = 1;
    pub const TRUNK_LIST_DETAIL: u8 = 2;
    pub const TRUNK_LIST: u8 = 3;
    pub const DECK_LIST: u8 = 4;
    pub const DECK_LIST_DETAIL: u8 = 5;
    pub const DECK_DETAIL: u8 = 6;
    pub const REINCARNATION_PROMPT: u8 = 7;
    pub const REINCARNATION_RESULTS: u8 = 8;
    pub const PASSWORD: u8 = 9;
    pub const MSG_PASSWORD_OK: u8 = 0xA;
    pub const MSG_PASSWORD_WRONG: u8 = 0xB;
    pub const MSG_PASSWORD_USED: u8 = 0xC;
    pub const MSG_DECK_INVALID: u8 = 0xD;
}

// ---------------------------------------------------------------------------
// Static card data

/// The static per-card data the editor reads: the 20-byte records
/// (0x1DE350), the language's name-order table and the password key. Read
/// from the user's executable at runtime, never embedded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticCards {
    /// 854 records of 20 bytes.
    pub records: Vec<[u8; RECORD_BYTES]>,
    /// u16 per card (sort key 1).
    pub name_order: Vec<u16>,
    /// The 8 key bytes at 0x2BE2F9.
    pub password_key: [u8; 8],
}

impl StaticCards {
    /// Slice the tables out of the PAL executable image (no authentication;
    /// `roses_assets` checks the hash). `language` is the byte at
    /// 0x314440 (0..4; others use table 0).
    pub fn from_executable(image: &[u8], language: u8) -> Option<Self> {
        let n = usize::from(CARD_COUNT);
        let table = image.get(elf::CARD_TABLE..elf::CARD_TABLE + n * RECORD_BYTES)?;
        let records = table
            .chunks_exact(RECORD_BYTES)
            .map(|r| r.try_into().expect("exact chunks"))
            .collect();
        let lang = if language < elf::LANGUAGES {
            usize::from(language)
        } else {
            0
        };
        let at = elf::NAME_ORDER + lang * elf::NAME_ORDER_STRIDE;
        let names = image.get(at..at + 2 * n)?;
        let name_order = names
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let password_key = image
            .get(elf::PASSWORD_KEY..elf::PASSWORD_KEY + 8)?
            .try_into()
            .ok()?;
        Some(Self {
            records,
            name_order,
            password_key,
        })
    }

    /// 0x1DE350: the record of a card id (0x29F → 999 → none; ids >= 854
    /// → none).
    pub fn record(&self, card: u16) -> Option<&[u8; RECORD_BYTES]> {
        let card = if card == REJECTED_DECK_CARD {
            EMPTY_CARD
        } else {
            card
        };
        self.records.get(usize::from(card))
    }
    /// Byte 0 >> 5 (0 = monster).
    pub fn kind(&self, card: u16) -> Option<u8> {
        self.record(card).map(|r| r[0] >> 5)
    }
    /// Byte 3.
    pub fn cost(&self, card: u16) -> Option<u8> {
        self.record(card).map(|r| r[3])
    }
    /// Byte 9 bit 6.
    pub fn has_password(&self, card: u16) -> bool {
        self.record(card).is_some_and(|r| r[9] & 0x40 != 0)
    }
    /// Byte 9 bit 7.
    pub fn reincarnation_eligible(&self, card: u16) -> bool {
        self.record(card).is_some_and(|r| r[9] & 0x80 != 0)
    }
    /// 0x1BB110: the 8-character password of a card (bytes 0xC..0x14
    /// decoded with the key), `None` when byte 9 bit 6 is clear.
    pub fn password(&self, card: u16) -> Option<[u8; 8]> {
        let r = self.record(card)?;
        if r[9] & 0x40 == 0 {
            return None;
        }
        let mut pw = [0u8; 8];
        for (j, out) in pw.iter_mut().enumerate() {
            let key = self.password_key[(usize::from(card) + j) & 7];
            *out = r[0xC + j].wrapping_add(key);
        }
        Some(pw)
    }
    /// 0x1BB110: the first card (by id) whose password equals `typed`.
    pub fn find_password(&self, typed: &[u8; 8]) -> Option<u16> {
        (0..CARD_COUNT).find(|&c| self.password(c).as_ref() == Some(typed))
    }
}

/// 0x1DF320(kind, attribute): the derived order of sort key 5 [emu:
/// tabulated for every kind 0..7 and attribute 0..15].
pub fn derived_order(kind: u8, attribute: u8) -> i32 {
    match kind {
        0 => i32::from(attribute),
        1 | 2 => 6,
        3 | 4 => 7,
        5 => 8,
        k => i32::from(k) + 5,
    }
}

// ---------------------------------------------------------------------------
// Card-block views: deck records, current deck, instance categories

/// One deck record (`BLK+0xFD94+d·0x54`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeckRecord {
    /// u16[40], 999 = empty slot.
    pub cards: [u16; DECK_SIZE],
    /// +0x50, 999 = none.
    pub leader: u16,
    /// +0x52, the deck cost.
    pub cost: u16,
}

impl DeckRecord {
    pub const EMPTY: Self = Self {
        cards: [EMPTY_CARD; DECK_SIZE],
        leader: EMPTY_CARD,
        cost: 0,
    };
    /// 0x215720: non-999 slots.
    pub fn count(&self) -> usize {
        self.cards.iter().filter(|&&c| c != EMPTY_CARD).count()
    }
    /// 0x1BB0A0: 40 cards and a leader.
    pub fn is_valid(&self) -> bool {
        self.count() >= DECK_SIZE && self.leader != EMPTY_CARD
    }
    /// 0x214960: the first slot holding the card.
    pub fn index_of(&self, card: u16) -> Option<usize> {
        self.cards.iter().position(|&c| c == card)
    }
}

/// Offset of deck `d` inside `CardBlock::raw_tail` (the tail starts at
/// +0xFD92; the records start at +0xFD94).
fn deck_tail_offset(d: u8) -> usize {
    2 + usize::from(d & 3) * 0x54
}
/// Offset of the current-deck byte (+0xFF9C) inside the raw tail.
const CURRENT_DECK_TAIL_OFFSET: usize = (0xFE90 - 0xFD92) + (0xFF98 - 0xFEA0);

fn tail_u16(block: &CardBlock, at: usize) -> u16 {
    match (block.raw_tail.get(at), block.raw_tail.get(at + 1)) {
        (Some(&lo), Some(&hi)) => u16::from_le_bytes([lo, hi]),
        _ => EMPTY_CARD,
    }
}
fn set_tail_u16(block: &mut CardBlock, at: usize, v: u16) {
    if at + 1 < block.raw_tail.len() {
        block.raw_tail[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }
}

/// Read deck record `d` (0..2) of the block. [F]
pub fn deck_record(block: &CardBlock, d: u8) -> DeckRecord {
    let base = deck_tail_offset(d);
    let mut cards = [EMPTY_CARD; DECK_SIZE];
    for (i, c) in cards.iter_mut().enumerate() {
        *c = tail_u16(block, base + 2 * i);
    }
    DeckRecord {
        cards,
        leader: tail_u16(block, base + 0x50),
        cost: tail_u16(block, base + 0x52),
    }
}

/// Write deck record `d` into the block's raw tail.
pub fn write_deck_record(block: &mut CardBlock, d: u8, record: &DeckRecord) {
    let base = deck_tail_offset(d);
    for (i, &c) in record.cards.iter().enumerate() {
        set_tail_u16(block, base + 2 * i, c);
    }
    set_tail_u16(block, base + 0x50, record.leader);
    set_tail_u16(block, base + 0x52, record.cost);
}

/// 0x215040: the current deck index (`BLK+0xFF9C & 3`).
pub fn current_deck(block: &CardBlock) -> u8 {
    block
        .raw_tail
        .get(CURRENT_DECK_TAIL_OFFSET)
        .map_or(0, |b| b & 3)
}

/// 0x2150B0: set the current deck (0..2; others ignored).
pub fn set_current_deck(block: &mut CardBlock, d: u8) -> bool {
    if d >= DECKS_PER_BLOCK {
        return false;
    }
    match block.raw_tail.get_mut(CURRENT_DECK_TAIL_OFFSET) {
        Some(b) => {
            *b = (*b & !3) | d;
            true
        }
        None => false,
    }
}

/// Bits 30–31 of word 1: 0/1/2 = in that deck, 3 = in the trunk.
pub fn instance_category(instance: CardInstance) -> u8 {
    (instance.id >> 30) as u8
}
/// Byte 3 bit 7 (= word 0 bit 31): this copy is a deck leader.
pub fn instance_is_leader(instance: CardInstance) -> bool {
    instance.owner & 0x8000_0000 != 0
}
fn set_category(instance: &mut CardInstance, category: u8) {
    instance.id = (instance.id & 0x3FFF_FFFF) | (u32::from(category & 3) << 30);
}
fn set_leader_flag(instance: &mut CardInstance, leader: bool) {
    instance.owner = (instance.owner & OWNER_KEY_MASK) | if leader { 0x8000_0000 } else { 0 };
}

fn record_index(card: u16) -> Option<usize> {
    let card = if card == REJECTED_DECK_CARD {
        EMPTY_CARD
    } else {
        card
    };
    (card < CARD_COUNT).then_some(usize::from(card))
}

/// 0x214560(side, card, category, leader): the first occupied instance
/// slot of the card with that category and leader flag.
pub fn find_instance(block: &CardBlock, card: u16, category: u8, leader: bool) -> Option<usize> {
    let record = block.records.get(record_index(card)?)?;
    record.instances.iter().position(|&i| {
        !i.is_free() && instance_category(i) == category && instance_is_leader(i) == leader
    })
}

/// 0x216250(side, card, category): occupied slots of the card, all of them
/// for `None`.
pub fn count_copies(block: &CardBlock, card: u16, category: Option<u8>) -> usize {
    let Some(record) = record_index(card).and_then(|i| block.records.get(i)) else {
        return 0;
    };
    record
        .instances
        .iter()
        .filter(|&&i| !i.is_free() && category.is_none_or(|c| instance_category(i) == c))
        .count()
}

/// 0x216350: byte +0x49 bit 6 (owned/seen).
pub fn seen(block: &CardBlock, card: u16) -> bool {
    block
        .record(card)
        .is_some_and(|r| r.rank_word & OWNED_BIT != 0)
}
/// 0x2164C0: byte +0x49 bit 7 (password used).
pub fn password_used(block: &CardBlock, card: u16) -> bool {
    block
        .record(card)
        .is_some_and(|r| r.rank_word & 0x8000 != 0)
}
/// 0x216580: set byte +0x49 bit 7.
pub fn set_password_used(block: &mut CardBlock, card: u16, used: bool) -> bool {
    let Some(record) = record_index(card).and_then(|i| block.records.get_mut(i)) else {
        return false;
    };
    record.rank_word = (record.rank_word & 0x7FFF) | if used { 0x8000 } else { 0 };
    true
}

/// 0x2154C0 as the editor uses it: the stored rank, −1 for ids the game
/// does not rank (>= 0x2AB, 0x29F).
fn rank_or_minus_one(block: &CardBlock, card: u16) -> i32 {
    block.stored_rank(card).map_or(-1, i32::from)
}

/// 0x214C70(side, deck, card): add one copy of the card to the deck. All
/// must hold: < 3 copies in the deck, a free slot, a trunk copy (category
/// 3, not a leader), a static record. Then `cost += REC[3]`, slot := card,
/// instance category := deck. [F][emu]
pub fn add_to_deck(block: &mut CardBlock, cards: &StaticCards, d: u8, card: u16) -> bool {
    if record_index(card).is_none() {
        return false;
    }
    let mut deck = deck_record(block, d);
    if deck.cards.iter().filter(|&&c| c == card).count() >= MAX_COPIES_IN_DECK {
        return false;
    }
    let Some(slot) = deck.index_of(EMPTY_CARD) else {
        return false;
    };
    let Some(instance) = find_instance(block, card, TRUNK_CATEGORY, false) else {
        return false;
    };
    let Some(cost) = cards.cost(card) else {
        return false;
    };
    deck.cost = deck.cost.wrapping_add(u16::from(cost));
    deck.cards[slot] = card;
    write_deck_record(block, d, &deck);
    let index = record_index(card).expect("checked above");
    set_category(&mut block.records[index].instances[instance], d & 3);
    true
}

/// 0x214F00(side, deck, slot): remove the card in the slot: `cost −=
/// REC[3]`, slot := 999, the first non-leader instance of the deck goes
/// back to the trunk. Nothing changes when no such instance or record
/// exists. [F][emu]
pub fn remove_from_deck(block: &mut CardBlock, cards: &StaticCards, d: u8, slot: usize) -> bool {
    if slot >= DECK_SIZE {
        return false;
    }
    let mut deck = deck_record(block, d);
    let card = deck.cards[slot];
    let Some(instance) = find_instance(block, card, d & 3, false) else {
        return false;
    };
    let Some(cost) = cards.cost(card) else {
        return false;
    };
    deck.cost = deck.cost.wrapping_sub(u16::from(cost));
    deck.cards[slot] = EMPTY_CARD;
    write_deck_record(block, d, &deck);
    let index = record_index(card).expect("found an instance");
    set_category(
        &mut block.records[index].instances[instance],
        TRUNK_CATEGORY,
    );
    true
}

/// 0x214A10(side, deck, card): make a trunk copy the deck leader. Refused
/// when the deck already has a leader or no trunk copy exists. [F][emu]
pub fn set_leader(block: &mut CardBlock, d: u8, card: u16) -> bool {
    let Some(index) = record_index(card) else {
        return false;
    };
    let mut deck = deck_record(block, d);
    if deck.leader != EMPTY_CARD {
        return false;
    }
    let Some(instance) = find_instance(block, card, TRUNK_CATEGORY, false) else {
        return false;
    };
    deck.leader = card;
    write_deck_record(block, d, &deck);
    let slot = &mut block.records[index].instances[instance];
    set_category(slot, d & 3);
    set_leader_flag(slot, true);
    true
}

/// 0x214B60(side, deck): send the leader back to the trunk. [F][emu]
pub fn remove_leader(block: &mut CardBlock, d: u8) -> bool {
    let mut deck = deck_record(block, d);
    if deck.leader == EMPTY_CARD {
        return false;
    }
    let Some(instance) = find_instance(block, deck.leader, d & 3, true) else {
        return false;
    };
    let index = record_index(deck.leader).expect("found an instance");
    deck.leader = EMPTY_CARD;
    write_deck_record(block, d, &deck);
    let slot = &mut block.records[index].instances[instance];
    set_category(slot, TRUNK_CATEGORY);
    set_leader_flag(slot, false);
    true
}

/// 0x215C20(side, card): delete one trunk copy (the sacrifice of a
/// reincarnation): owner := 0, word 1 := 0xC0000000. [F][emu]
pub fn delete_trunk_copy(block: &mut CardBlock, card: u16) -> bool {
    let Some(instance) = find_instance(block, card, TRUNK_CATEGORY, false) else {
        return false;
    };
    let index = record_index(card).expect("found an instance");
    block.records[index].instances[instance] = CardInstance {
        owner: 0,
        id: 0xC000_0000,
    };
    true
}

/// 0x2153D0(side, deck): recompute and store the deck cost. [F]
pub fn recompute_deck_cost(block: &mut CardBlock, cards: &StaticCards, d: u8) -> u16 {
    let mut deck = deck_record(block, d);
    deck.cost = deck
        .cards
        .iter()
        .filter_map(|&c| cards.cost(c))
        .fold(0u16, |acc, c| acc.wrapping_add(u16::from(c)));
    write_deck_record(block, d, &deck);
    deck.cost
}

/// Lay out a new game's deck inside a block whose cards were just granted
/// (one instance per deck card plus the leader): every occupied instance
/// goes to the trunk, the other two deck records are emptied (999s), the
/// first 40 `cards` take one instance each into deck `d` (in the given
/// order, 999 padding), the leader's instance gets the leader flag, the
/// cost is recomputed (0x2153D0) and `d` becomes the current deck. Free
/// slots get word 1 = 0xC0000000 as the capture shows them. The original's
/// new-game writer is untraced [H]; the result is the layout the capture of
/// the first duel's player block shows (deck instances category 0, the
/// leader's flag, empty decks 1 and 2, 0xC0000000 free slots) and the
/// editor's traced readers (0x214560, 0x216250, 0x215160/0x2151F0) expect.
/// Errors name the card that has no spare instance.
pub fn lay_out_new_game_deck(
    block: &mut CardBlock,
    cards: &StaticCards,
    d: u8,
    deck_cards: &[u16],
    leader: u16,
) -> Result<(), String> {
    if d >= DECKS_PER_BLOCK {
        return Err(format!("deck {d} is not one of the three decks"));
    }
    if deck_cards.len() > DECK_SIZE {
        return Err(format!(
            "{} cards do not fit a 40-card deck",
            deck_cards.len()
        ));
    }
    for record in &mut block.records {
        for slot in &mut record.instances {
            if slot.is_free() {
                slot.id = 0xC000_0000;
            } else {
                set_category(slot, TRUNK_CATEGORY);
                set_leader_flag(slot, false);
            }
        }
    }
    // All three records start empty (999s, leader 999, cost 0) as the
    // capture's new-game block holds them; an all-zero record would read
    // as "card 0 ×40 with leader 0", a valid deck.
    for k in 0..DECKS_PER_BLOCK {
        write_deck_record(block, k, &DeckRecord::EMPTY);
    }
    let mut record = DeckRecord::EMPTY;
    for (i, &card) in deck_cards.iter().enumerate() {
        let index = record_index(card).ok_or_else(|| format!("card {card} has no record"))?;
        let instance = find_instance(block, card, TRUNK_CATEGORY, false)
            .ok_or_else(|| format!("card {card} has no spare instance for the deck"))?;
        set_category(&mut block.records[index].instances[instance], d);
        record.cards[i] = card;
    }
    write_deck_record(block, d, &record);
    if !set_leader(block, d, leader) {
        return Err(format!("leader {leader} has no spare instance"));
    }
    recompute_deck_cost(block, cards, d);
    set_current_deck(block, d);
    Ok(())
}

/// 0x1BB0A0(side, deck).
pub fn deck_is_valid(block: &CardBlock, d: u8) -> bool {
    deck_record(block, d).is_valid()
}

// ---------------------------------------------------------------------------
// Sorting (qsort 0x19A480 with 0x1BB320 / 0x1BB400 and the comparators
// 0x2905B0[k])

/// Compare two list entries under one sort key. Keys 2 and 3 (ATK, DEF)
/// are descending, every other key ascending; a missing record (999)
/// takes the value each comparator assigns it. [F][emu: key cycling
/// 0..9 on the trunk list]
pub fn compare_key(key: u8, block: &CardBlock, cards: &StaticCards, a: u16, b: u16) -> Ordering {
    const MAX: i32 = 0x7FFF_FFFF;
    let value = |c: u16| -> i32 {
        let rec = cards.record(c);
        match key {
            0 => {
                if c == EMPTY_CARD {
                    2
                } else if block.recent.contains(&c) {
                    0
                } else {
                    1
                }
            }
            1 => {
                if c == EMPTY_CARD {
                    MAX
                } else {
                    cards
                        .name_order
                        .get(usize::from(c))
                        .map_or(MAX, |&n| i32::from(n))
                }
            }
            2 => rec.map_or(-1, |r| i32::from(u16::from_le_bytes([r[8], r[9]]) & 0x1FFF)),
            3 => rec.map_or(-1, |r| {
                i32::from(u16::from_le_bytes([r[0xA], r[0xB]]) & 0x1FFF)
            }),
            4 => rec.map_or(MAX, |r| i32::from(r[0])),
            5 => rec.map_or(MAX, |r| derived_order(r[0] >> 5, r[2] & 0xF)),
            6 => rec.map_or(MAX, |r| i32::from(r[2] >> 4)),
            7 => rec.map_or(MAX, |r| i32::from(r[3])),
            8 => {
                if c < RANKED_CARDS {
                    rank_or_minus_one(block, c)
                } else {
                    MAX
                }
            }
            9 => rec.map_or(MAX, |r| {
                let kind = i32::from(r[0] >> 5);
                if kind != 0 {
                    kind + 6
                } else if u16::from_le_bytes([r[4], r[5]]) != 0xFFFF {
                    0
                } else {
                    6
                }
            }),
            _ => i32::from(c),
        }
    };
    let (va, vb) = (value(a), value(b));
    if key == 2 || key == 3 {
        vb.cmp(&va)
    } else {
        va.cmp(&vb)
    }
}

/// 0x1BB320 / 0x1BB400: the three keys in order, a repeated key skipped.
pub fn compare_keys(
    keys: &[u8; 3],
    block: &CardBlock,
    cards: &StaticCards,
    a: u16,
    b: u16,
) -> Ordering {
    for (i, &key) in keys.iter().enumerate() {
        if keys[..i].contains(&key) {
            continue;
        }
        let order = compare_key(key, block, cards, a, b);
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

fn sort_list(list: &mut [u16], keys: &[u8; 3], block: &CardBlock, cards: &StaticCards) {
    list.sort_by(|&a, &b| compare_keys(keys, block, cards, a, b));
}

// ---------------------------------------------------------------------------
// Reincarnation (0x251D50, 0x251F40)

/// 0x251F40(side, cost): the target cost. `m` = the highest stored rank of
/// the leaders of decks 0 and 1 (not 2 [F]); with `2(m−1) + rand%100 <
/// 90` the target is `cost + 1 − rand%12`, else `cost + 1 + rand%10`.
pub fn reincarnation_target_cost(block: &CardBlock, cost: u8, rng: &mut u64) -> i32 {
    let mut m = 0i32;
    for d in 0..2 {
        let leader = deck_record(block, d).leader;
        if leader != EMPTY_CARD {
            let r = rank_or_minus_one(block, leader);
            if m < r {
                m = r;
            }
        }
    }
    let roll = (next_rand31(rng) % 100) as i32;
    let cost = i32::from(cost);
    if 2 * (m - 1) + roll < 90 {
        cost + 1 - (next_rand31(rng) % 12) as i32
    } else {
        cost + 1 + (next_rand31(rng) % 10) as i32
    }
}

/// 0x251D50(side, card): one reincarnation result. Pool = monsters (ids <
/// 683) with probability 80 % for a monster sacrifice, 60 % otherwise,
/// else spells/traps; candidates = cards other than `card` of the pool
/// with byte 3 == target and byte 9 bit 7; the target decrements until a
/// candidate exists, 820 when it reaches 0 (or starts <= 0). [F][emu]
pub fn reincarnate_one(block: &CardBlock, cards: &StaticCards, card: u16, rng: &mut u64) -> u16 {
    let Some(rec) = cards.record(card) else {
        return REINCARNATION_FALLBACK;
    };
    let roll = next_rand31(rng) % 10;
    let monsters = if rec[0] >> 5 == 0 {
        roll < 8
    } else {
        roll >= 4
    };
    let mut target = reincarnation_target_cost(block, rec[3], rng);
    if target <= 0 {
        return REINCARNATION_FALLBACK;
    }
    let pool = if monsters {
        0..RANKED_CARDS
    } else {
        RANKED_CARDS..CARD_COUNT
    };
    loop {
        let candidates: Vec<u16> = pool
            .clone()
            .filter(|&c| c != card)
            .filter(|&c| {
                cards
                    .record(c)
                    .is_some_and(|r| i32::from(r[3]) == target && r[9] & 0x80 != 0)
            })
            .collect();
        if !candidates.is_empty() {
            let pick = next_rand31(rng) as usize % candidates.len();
            return candidates[pick];
        }
        target -= 1;
        if target == 0 {
            return REINCARNATION_FALLBACK;
        }
    }
}

// ---------------------------------------------------------------------------
// Pad

/// Per-frame input: the pad word and, when an analog pad is present, the
/// right-stick and left-stick Y bytes (0x203580(0, 1) / (0, 3); 0x80 =
/// centred).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadInput {
    pub word: u16,
    pub sticks: Option<Sticks>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sticks {
    pub right_y: u8,
    pub left_y: u8,
}

/// The pad driver's view of port 0: current and previous word and the
/// 16 auto-repeat counters of 0x203380 kind 2 / 0x203490 [F]: the frame
/// routine 0x203180 advances a button's counter when it was down in both
/// samples *before* the new word arrives (the older pair), otherwise sets
/// it to minus the delay (pad +0x59); kind 2 reports a new press, or any
/// button of the mask whose counter is >= 0 and a multiple of the rate —
/// the rate is the low nibble of pad +0x5A, sign-extended (0x203418:
/// `dsll32 28; dsra32 28`), so the captured byte 33 (0x21) is a rate of 1.
/// The first repeat comes 13 frames after the press, then one every frame,
/// and one more on the frame the release is first seen (as the duel's
/// `PadRepeat`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadState {
    pub held: u16,
    pub previous: u16,
    pub counters: [i32; 16],
    /// Byte +0x59 of the pad state (12 in the capture).
    pub repeat_delay: i32,
    /// Byte +0x5A (33 in the capture).
    pub repeat_rate: i32,
}

impl Default for PadState {
    fn default() -> Self {
        Self {
            held: 0,
            previous: 0,
            counters: [0; 16],
            repeat_delay: 12,
            repeat_rate: 33,
        }
    }
}

impl PadState {
    /// Advance to the next frame's word (0x203180): the counters from the
    /// two samples before it, then the words shift.
    pub fn poll(&mut self, word: u16) {
        let steady = self.held & self.previous;
        for bit in 0..16 {
            if steady & (1u16 << bit) != 0 {
                self.counters[bit] = self.counters[bit].saturating_add(1);
            } else {
                self.counters[bit] = -self.repeat_delay;
            }
        }
        self.previous = self.held;
        self.held = word;
    }
    /// The repeat period: pad +0x5A's low nibble, sign-extended [F].
    fn rate(&self) -> i32 {
        (self.repeat_rate << 28) >> 28
    }
    /// 0x203380 kind 0: newly pressed bits of the mask.
    pub fn new_press(&self, mask: u16) -> bool {
        self.held & !self.previous & mask != 0
    }
    /// 0x203380 kind 1: held bits of the mask.
    pub fn is_held(&self, mask: u16) -> bool {
        self.held & mask != 0
    }
    /// 0x203380 kind 2: newly pressed, or a counter >= 0 that is a
    /// multiple of the rate (the held word is not tested [F]).
    pub fn repeat(&self, mask: u16) -> bool {
        if self.new_press(mask) {
            return true;
        }
        let rate = self.rate();
        (0..16).any(|bit| {
            let m = 1u16 << bit;
            m & mask != 0 && self.counters[bit] >= 0 && rate != 0 && self.counters[bit] % rate == 0
        })
    }
    /// 0x203490: restart the repeat delay of the mask's bits.
    pub fn restart(&mut self, mask: u16) {
        for bit in 0..16 {
            if mask & (1u16 << bit) != 0 {
                self.counters[bit] = -self.repeat_delay;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The model

/// What the update emits besides state changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorEvent {
    /// 0x216EA0(0, id).
    Sound(u8),
    /// 0x1C1F40(card) then push mode 7 with parameter 2 (the 3D viewer).
    Viewer(u16),
    /// 0x1A9290(): the mode is popped; the editor is finished.
    Exit,
    /// 0x2022F0(2, side): release the card-graphics cache (presentation).
    ReleaseGraphics,
    /// 0x200320(side + 5, 1): leader graphics reloaded (presentation).
    ReloadLeader,
    /// 0x1BB210: an animation slot started (presentation): `kind` is the
    /// a2 argument (1 = add / leader), `leader_removed` a3, `removed` t0.
    Animation {
        card: u16,
        row: u8,
        kind: bool,
        leader_removed: bool,
        removed: bool,
    },
    /// 0x2157D0 succeeded for a password or reincarnation result.
    Granted(u16),
    /// 0x23BCC0: `SAVE+0xCA` := 0.
    DuelCounterReset,
}

/// The persistent data and services the update needs each frame.
pub struct EditorEnv<'a> {
    /// The side's card block (`BLK(side)`).
    pub block: &'a mut CardBlock,
    /// `SAVE+0xCA`, the campaign duel counter (0x23BCB0 / 0x23BCC0).
    pub duel_counter: &'a mut u8,
    pub cards: &'a StaticCards,
    /// The shared LCG state (0x19AE88). The main loop's own calls (2 per
    /// frame) are outside this model.
    pub rng: &'a mut u64,
    /// 0x1FEC40() != 0: graphics still loading (blocks reincarnation and
    /// the leader reload).
    pub graphics_busy: bool,
}

/// A view of the editor state for callers and tests (addresses of §2 of
/// the research).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorSnapshot {
    pub state: u8,
    pub return_state: u8,
    pub slide: i16,
    pub slide_target: i16,
    pub trunk_count: u16,
    pub trunk_window: u16,
    pub trunk_row: u8,
    pub deck_window: u16,
    pub deck_row: u8,
    pub trunk_keys: [u8; 3],
    pub deck_keys: [u8; 3],
    pub trunk_page: u8,
    pub deck_page: u8,
    pub catalogue: [u8; 3],
    pub counter: u8,
    pub sub_step: u8,
    pub password: [u8; 8],
    pub idle: u16,
    pub reload_pending: bool,
    pub results: [u16; 3],
    /// The sorted trunk list, owned kinds only.
    pub trunk: Vec<u16>,
    /// Leader row + 40 slots, sorted from the second entry.
    pub deck_list: Vec<u16>,
}

/// The editor of one side, frame-stepped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeckEditor {
    side: u8,
    state: u8,
    return_state: u8,
    slide: i16,
    slide_target: i16,
    trunk: Vec<u16>,
    trunk_count: u16,
    deck_list: [u16; DECK_LIST_LEN],
    trunk_window: u16,
    trunk_row: u8,
    deck_window: u16,
    deck_row: u8,
    trunk_keys: [u8; 3],
    deck_keys: [u8; 3],
    trunk_page: u8,
    deck_page: u8,
    /// Page base, column, row (0x2DB900..902).
    catalogue: [u8; 3],
    /// 0x2DB8D6: yes/no, sub-step frame counter or password cursor.
    counter: u8,
    sub_step: u8,
    password: [u8; 8],
    idle: u16,
    reload_pending: bool,
    results: [u16; 3],
    pad: PadState,
    frame: u64,
    finished: bool,
}

/// Which of the two lists an operation addressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListSide {
    Trunk,
    Deck,
}

impl DeckEditor {
    /// 0x1B9100(side): build both lists for the block's current deck,
    /// sort them with the default keys, recompute the current deck's cost,
    /// state 3. [F][emu]
    pub fn new(side: u8, env: &mut EditorEnv<'_>) -> Self {
        let mut editor = Self {
            side,
            state: state::TRUNK_LIST,
            return_state: 0,
            slide: 0,
            slide_target: 0,
            trunk: vec![EMPTY_CARD; usize::from(CARD_COUNT)],
            trunk_count: 0,
            deck_list: [EMPTY_CARD; DECK_LIST_LEN],
            trunk_window: 0,
            trunk_row: 0,
            deck_window: 0,
            deck_row: 0,
            trunk_keys: DEFAULT_SORT_KEYS,
            deck_keys: DEFAULT_SORT_KEYS,
            trunk_page: 0,
            deck_page: 0,
            catalogue: [0; 3],
            counter: 0,
            sub_step: 0,
            // 0x1B9100 does not write the buffer (zeros in the capture
            // [emu]); R3 resets it to "00000000".
            password: [0; 8],
            idle: 0,
            reload_pending: false,
            results: [0; 3],
            pad: PadState::default(),
            frame: 0,
            finished: false,
        };
        let d = current_deck(env.block);
        editor.fill_trunk(env.block);
        editor.fill_deck_list(env.block, d);
        editor.sort_trunk(env);
        editor.sort_deck(env);
        recompute_deck_cost(env.block, env.cards, d);
        editor
    }

    pub fn side(&self) -> u8 {
        self.side
    }
    pub fn state(&self) -> u8 {
        self.state
    }
    /// Frames stepped so far.
    pub fn frame(&self) -> u64 {
        self.frame
    }
    /// The mode was popped (○ with a valid deck); further steps do nothing.
    pub fn finished(&self) -> bool {
        self.finished
    }
    pub fn pad(&self) -> &PadState {
        &self.pad
    }
    /// The sorted trunk list (owned kinds).
    pub fn trunk_list(&self) -> &[u16] {
        &self.trunk[..usize::from(self.trunk_count)]
    }
    /// Leader row + 40 slots.
    pub fn deck_list(&self) -> &[u16; DECK_LIST_LEN] {
        &self.deck_list
    }
    /// The card under the cursor of a list.
    pub fn highlighted(&self, list: ListSide) -> u16 {
        match list {
            ListSide::Trunk => self.trunk_card(),
            ListSide::Deck => self.deck_list[self.deck_index()],
        }
    }

    pub fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            state: self.state,
            return_state: self.return_state,
            slide: self.slide,
            slide_target: self.slide_target,
            trunk_count: self.trunk_count,
            trunk_window: self.trunk_window,
            trunk_row: self.trunk_row,
            deck_window: self.deck_window,
            deck_row: self.deck_row,
            trunk_keys: self.trunk_keys,
            deck_keys: self.deck_keys,
            trunk_page: self.trunk_page,
            deck_page: self.deck_page,
            catalogue: self.catalogue,
            counter: self.counter,
            sub_step: self.sub_step,
            password: self.password,
            idle: self.idle,
            reload_pending: self.reload_pending,
            results: self.results,
            trunk: self.trunk_list().to_vec(),
            deck_list: self.deck_list.to_vec(),
        }
    }

    // --- list buffers --------------------------------------------------

    /// Trunk list = every card id with >= 1 instance, 999 otherwise.
    fn fill_trunk(&mut self, block: &CardBlock) {
        self.trunk_count = 0;
        for c in 0..CARD_COUNT {
            let owned = count_copies(block, c, None) > 0;
            self.trunk[usize::from(c)] = if owned { c } else { EMPTY_CARD };
            if owned {
                self.trunk_count += 1;
            }
        }
    }
    fn fill_deck_list(&mut self, block: &CardBlock, d: u8) {
        let record = deck_record(block, d);
        self.deck_list[0] = record.leader;
        self.deck_list[1..].copy_from_slice(&record.cards);
    }
    /// 0x1BB2E0: sort all 854 trunk entries.
    fn sort_trunk(&mut self, env: &EditorEnv<'_>) {
        sort_list(&mut self.trunk, &self.trunk_keys, env.block, env.cards);
    }
    /// 0x1BB300: sort the 40 slots after the leader row.
    fn sort_deck(&mut self, env: &EditorEnv<'_>) {
        sort_list(
            &mut self.deck_list[1..],
            &self.deck_keys,
            env.block,
            env.cards,
        );
    }
    /// 0x1B9390: rebuild the trunk list after a grant, clamp its window,
    /// re-sort.
    fn rebuild_trunk(&mut self, env: &EditorEnv<'_>) {
        self.fill_trunk(env.block);
        let max = self.trunk_count.saturating_sub(VISIBLE_ROWS);
        self.trunk_window = self.trunk_window.min(max);
        self.sort_trunk(env);
    }
    fn trunk_card(&self) -> u16 {
        self.trunk
            .get(usize::from(self.trunk_window) + usize::from(self.trunk_row))
            .copied()
            .unwrap_or(EMPTY_CARD)
    }
    fn deck_index(&self) -> usize {
        (usize::from(self.deck_window) + usize::from(self.deck_row)).min(DECK_LIST_LEN - 1)
    }
    fn with_detail(&self) -> bool {
        self.state == state::TRUNK_LIST_DETAIL || self.state == state::DECK_LIST_DETAIL
    }

    // --- stepping ------------------------------------------------------

    /// One frame of 0x1B94B0. While the pane slides no input is read; the
    /// idle timer, the state's input handling and the window scrolling
    /// follow. Returns the frame's events in order.
    pub fn step(&mut self, input: PadInput, env: &mut EditorEnv<'_>) -> Vec<EditorEvent> {
        let mut ev = Vec::new();
        if self.finished {
            return ev;
        }
        self.frame += 1;
        let d = current_deck(env.block);
        if !env.graphics_busy && self.reload_pending {
            ev.push(EditorEvent::ReloadLeader);
            self.reload_pending = false;
        }
        self.pad.poll(input.word);
        if self.slide != self.slide_target {
            let delta = (self.slide_target - self.slide).clamp(-SLIDE_STEP, SLIDE_STEP);
            self.slide += delta;
            return ev;
        }
        if self.pad.new_press(0xFFFF) {
            self.idle = IDLE_FRAMES;
        } else if self.idle != 0 {
            self.idle -= 1;
        }
        let mut scroll = Scroll::default();
        match self.state {
            state::CATALOGUE => self.catalogue_state(env, &mut ev),
            state::TRUNK_DETAIL => self.trunk_detail_state(&mut ev),
            state::TRUNK_LIST_DETAIL | state::TRUNK_LIST => {
                self.trunk_list_state(d, env, &mut ev, &mut scroll)
            }
            state::DECK_LIST | state::DECK_LIST_DETAIL => {
                self.deck_list_state(d, env, &mut ev, &mut scroll)
            }
            state::DECK_DETAIL => self.deck_detail_state(&mut ev),
            state::REINCARNATION_PROMPT => self.reincarnation_prompt_state(env, &mut ev),
            state::REINCARNATION_RESULTS => self.reincarnation_results_state(env, &mut ev),
            state::PASSWORD => self.password_state(env, &mut ev),
            state::MSG_PASSWORD_OK..=state::MSG_DECK_INVALID => {
                if self.pad.new_press(0xF0) {
                    self.state = self.return_state;
                    ev.push(EditorEvent::Sound(sound::ACCEPT));
                }
            }
            _ => {}
        }
        self.tail(input.sticks, scroll, &mut ev);
        ev
    }

    /// 0x1BAEAC..0x1BB070: analog-stick scrolling and the window moves.
    fn tail(&mut self, sticks: Option<Sticks>, mut scroll: Scroll, ev: &mut Vec<EditorEvent>) {
        if let Some(sticks) = sticks {
            if (1..=4).contains(&self.state) && scroll.trunk == 0 {
                scroll.trunk = stick_rows(sticks.left_y, 18);
            }
            if (3..=6).contains(&self.state) && scroll.deck == 0 {
                scroll.deck = stick_rows(sticks.right_y, 6);
            }
        }
        if scroll.trunk != 0 {
            let max = i32::from(self.trunk_count.saturating_sub(VISIBLE_ROWS));
            let old = self.trunk_window;
            self.trunk_window = (i32::from(old) + scroll.trunk).clamp(0, max) as u16;
            if self.trunk_window != old {
                ev.push(EditorEvent::Sound(sound::CURSOR));
            }
            self.idle = IDLE_FRAMES;
        }
        if scroll.deck != 0 {
            let max = (DECK_LIST_LEN as i32 - i32::from(VISIBLE_ROWS)).max(0);
            let old = self.deck_window;
            self.deck_window = (i32::from(old) + scroll.deck).clamp(0, max) as u16;
            if self.deck_window != old {
                ev.push(EditorEvent::Sound(sound::CURSOR));
            }
            self.idle = IDLE_FRAMES;
        }
    }

    /// State 0: all 854 ids in rows of 10.
    fn catalogue_state(&mut self, env: &EditorEnv<'_>, ev: &mut Vec<EditorEvent>) {
        if self.pad.new_press(pad::CIRCLE) {
            self.state = state::TRUNK_DETAIL;
            self.slide_target = 0x100;
            ev.push(EditorEvent::Sound(sound::CLOSE));
            return;
        }
        if self.pad.repeat(0xF00C) {
            let before = self.catalogue;
            let mut page_delta: i32 = 0;
            if self.pad.repeat(pad::L1 | pad::R1) {
                page_delta = if self.pad.repeat(pad::L1) { -12 } else { 12 };
            } else if self.pad.repeat(pad::UP | pad::DOWN) {
                if self.pad.repeat(pad::UP) {
                    if self.catalogue[2] > 0 {
                        self.catalogue[2] -= 1;
                    } else {
                        page_delta = -1;
                    }
                } else if self.catalogue[2] < 11 {
                    self.catalogue[2] += 1;
                } else {
                    page_delta = 1;
                }
            }
            if page_delta != 0 {
                self.catalogue[0] = (i32::from(self.catalogue[0]) + page_delta)
                    .clamp(0, i32::from(CATALOGUE_MAX_BASE))
                    as u8;
            }
            if self.pad.repeat(pad::LEFT | pad::RIGHT) {
                self.catalogue[1] = if self.pad.repeat(pad::LEFT) {
                    (self.catalogue[1] + 9) % 10
                } else {
                    (self.catalogue[1] + 11) % 10
                };
            }
            if self.catalogue != before {
                ev.push(EditorEvent::Sound(sound::CURSOR));
            }
            return;
        }
        let card = self.catalogue_card();
        let known = |limit: u16| {
            card < limit && (count_copies(env.block, card, None) > 0 || seen(env.block, card))
        };
        if self.pad.repeat(pad::L2 | pad::R2) {
            if known(CARD_COUNT) {
                let before = self.trunk_page;
                self.trunk_page = if self.pad.repeat(pad::L2) {
                    (self.trunk_page + 3) % 4
                } else {
                    (self.trunk_page + 5) % 4
                };
                if self.trunk_page != before {
                    ev.push(EditorEvent::Sound(sound::SORT));
                }
            } else {
                ev.push(EditorEvent::Sound(sound::REFUSED));
            }
            return;
        }
        if self.pad.new_press(pad::SQUARE | pad::CROSS | pad::TRIANGLE) {
            if known(RANKED_CARDS) {
                ev.push(EditorEvent::Viewer(card));
                ev.push(EditorEvent::Sound(sound::ACCEPT));
            } else {
                ev.push(EditorEvent::Sound(sound::REFUSED));
            }
        }
    }

    /// The catalogue cursor's card: column + 10·(base + row), 0x29F → 999.
    pub fn catalogue_card(&self) -> u16 {
        let card = u16::from(self.catalogue[1])
            + 10 * (u16::from(self.catalogue[0]) + u16::from(self.catalogue[2]));
        if card == REJECTED_DECK_CARD {
            EMPTY_CARD
        } else {
            card
        }
    }

    /// State 1: the trunk-card detail pane.
    fn trunk_detail_state(&mut self, ev: &mut Vec<EditorEvent>) {
        if self.pad.new_press(pad::START) {
            self.state = state::CATALOGUE;
            self.slide_target = 0x200;
            ev.push(EditorEvent::Sound(sound::OPEN));
            return;
        }
        if self.pad.repeat(pad::UP | pad::DOWN) {
            let before = self.trunk_page;
            self.trunk_page = if self.pad.repeat(pad::UP) {
                (self.trunk_page + 3) % 4
            } else {
                (self.trunk_page + 5) % 4
            };
            if self.trunk_page != before {
                ev.push(EditorEvent::Sound(sound::SORT));
            }
            self.pad.restart(pad::LEFT | pad::RIGHT);
            return;
        }
        if self.pad.repeat(pad::RIGHT) {
            self.state = state::TRUNK_LIST_DETAIL;
            ev.push(EditorEvent::Sound(sound::PANE));
            return;
        }
        if self.pad.new_press(pad::CIRCLE) {
            self.state = state::TRUNK_LIST_DETAIL;
            ev.push(EditorEvent::Sound(sound::CLOSE));
        }
    }

    /// States 2 and 3: the trunk list.
    fn trunk_list_state(
        &mut self,
        d: u8,
        env: &mut EditorEnv<'_>,
        ev: &mut Vec<EditorEvent>,
        scroll: &mut Scroll,
    ) {
        let card = self.trunk_card();
        let slot = deck_record(env.block, d).index_of(card);
        let select = self.pad.is_held(pad::SELECT);
        let row_before = self.trunk_row;
        if self.pad.new_press(pad::L3) {
            let mut ok = count_copies(env.block, card, Some(TRUNK_CATEGORY)) > 0;
            if env.graphics_busy || *env.duel_counter < REINCARNATION_DUELS {
                ok = false;
            }
            if !(0..DECKS_PER_BLOCK).any(|k| deck_is_valid(env.block, k)) {
                ok = false;
            }
            if ok {
                self.counter = 0;
                self.return_state = self.state;
                self.state = state::REINCARNATION_PROMPT;
                ev.push(EditorEvent::Sound(sound::OPEN));
            } else {
                ev.push(EditorEvent::Sound(sound::REFUSED));
            }
            return;
        }
        if self.pad.new_press(pad::R3) {
            self.password = *b"00000000";
            self.counter = 0;
            self.return_state = self.state;
            self.state = state::PASSWORD;
            ev.push(EditorEvent::Sound(sound::OPEN));
            return;
        }
        if self.pad.repeat(pad::L1 | pad::R1) {
            scroll.trunk = if self.pad.repeat(pad::L1) { -6 } else { 6 };
        } else if self.pad.repeat(pad::UP | pad::DOWN) {
            if self.pad.repeat(pad::UP) {
                if self.trunk_row > 0 {
                    self.trunk_row -= 1;
                } else {
                    scroll.trunk = -1;
                }
            } else if self.trunk_row < 5 {
                self.trunk_row += 1;
            } else {
                scroll.trunk = 1;
            }
            self.pad.restart(pad::LEFT | pad::RIGHT);
        }
        if self.trunk_row != row_before {
            ev.push(EditorEvent::Sound(sound::CURSOR));
            return;
        }
        if scroll.trunk != 0 {
            return;
        }
        if self.pad.new_press(pad::START) {
            if rank_or_minus_one(env.block, card) > 0 {
                if count_copies(env.block, card, Some(TRUNK_CATEGORY)) > 0 {
                    if self.deck_list[0] != EMPTY_CARD {
                        remove_leader(env.block, d);
                        self.deck_list[0] = EMPTY_CARD;
                    }
                    if set_leader(env.block, d, card) {
                        self.deck_list[0] = card;
                        ev.push(EditorEvent::Animation {
                            card,
                            row: self.trunk_row,
                            kind: true,
                            leader_removed: false,
                            removed: false,
                        });
                        ev.push(EditorEvent::ReleaseGraphics);
                        self.reload_pending = true;
                        ev.push(EditorEvent::Sound(sound::ACCEPT));
                    } else {
                        ev.push(EditorEvent::Sound(sound::REFUSED));
                    }
                } else {
                    ev.push(EditorEvent::Sound(sound::REFUSED));
                }
            } else {
                ev.push(EditorEvent::Sound(sound::REFUSED));
            }
            return;
        }
        if self.pad.repeat(pad::SQUARE | pad::CROSS) {
            if self.pad.repeat(pad::SQUARE) {
                let removed = slot.is_some_and(|s| remove_from_deck(env.block, env.cards, d, s));
                if removed {
                    if let Some(i) = self.deck_list[1..].iter().position(|&c| c == card) {
                        self.deck_list[1 + i] = EMPTY_CARD;
                    }
                    self.sort_deck(env);
                    ev.push(EditorEvent::Animation {
                        card,
                        row: self.trunk_row,
                        kind: false,
                        leader_removed: false,
                        removed: true,
                    });
                    ev.push(EditorEvent::Sound(sound::ACCEPT));
                } else {
                    ev.push(EditorEvent::Sound(sound::REFUSED));
                }
            } else if add_to_deck(env.block, env.cards, d, card) {
                self.deck_list[DECK_LIST_LEN - 1] = card;
                self.sort_deck(env);
                ev.push(EditorEvent::Animation {
                    card,
                    row: self.trunk_row,
                    kind: true,
                    leader_removed: false,
                    removed: false,
                });
                ev.push(EditorEvent::Sound(sound::ACCEPT));
            } else {
                ev.push(EditorEvent::Sound(sound::REFUSED));
            }
            return;
        }
        if self.pad.repeat(pad::L2 | pad::R2) {
            let k = usize::from(select);
            let before = self.trunk_keys[k];
            self.trunk_keys[k] = cycle_key(before, self.pad.repeat(pad::L2));
            if self.trunk_keys[k] != before {
                self.sort_trunk(env);
                ev.push(EditorEvent::Sound(sound::SORT));
            }
            return;
        }
        if self.pad.new_press(pad::CIRCLE) {
            if self.with_detail() {
                self.state = state::TRUNK_LIST;
                self.slide_target = 0;
                ev.push(EditorEvent::Sound(sound::CLOSE));
            } else {
                self.try_exit(d, env, ev);
            }
            return;
        }
        if self.pad.new_press(pad::TRIANGLE) {
            if self.with_detail() {
                self.viewer(card, env, ev);
            } else {
                self.state = state::TRUNK_LIST_DETAIL;
                self.slide_target = 0x100;
                ev.push(EditorEvent::Sound(sound::OPEN));
            }
            return;
        }
        if self.pad.repeat(pad::LEFT | pad::RIGHT) {
            if self.pad.repeat(pad::LEFT) {
                if self.with_detail() {
                    self.state = state::TRUNK_DETAIL;
                } else {
                    self.state = state::TRUNK_LIST_DETAIL;
                    self.slide_target = 0x100;
                }
            } else if self.with_detail() {
                self.state = state::TRUNK_LIST;
                self.slide_target = 0;
            } else {
                self.state = state::DECK_LIST;
                self.deck_row = self.trunk_row;
            }
            ev.push(EditorEvent::Sound(sound::PANE));
        }
    }

    /// States 4 and 5: the deck list.
    fn deck_list_state(
        &mut self,
        d: u8,
        env: &mut EditorEnv<'_>,
        ev: &mut Vec<EditorEvent>,
        scroll: &mut Scroll,
    ) {
        let index = self.deck_index();
        let card = self.deck_list[index];
        let slot = deck_record(env.block, d).index_of(card);
        let select = self.pad.is_held(pad::SELECT);
        let row_before = self.deck_row;
        if self.pad.repeat(pad::L1 | pad::R1) {
            scroll.deck = if self.pad.repeat(pad::L1) { -6 } else { 6 };
        } else if self.pad.repeat(pad::UP | pad::DOWN) {
            if self.pad.repeat(pad::UP) {
                if self.deck_row > 0 {
                    self.deck_row -= 1;
                } else {
                    scroll.deck = -1;
                }
            } else if self.deck_row < 5 {
                self.deck_row += 1;
            } else {
                scroll.deck = 1;
            }
            self.pad.restart(pad::LEFT | pad::RIGHT);
        }
        if self.deck_row != row_before {
            ev.push(EditorEvent::Sound(sound::CURSOR));
            return;
        }
        if scroll.deck != 0 {
            return;
        }
        if self.pad.new_press(pad::START) {
            let next = (d + 1) % DECKS_PER_BLOCK;
            set_current_deck(env.block, next);
            let now = current_deck(env.block);
            self.fill_deck_list(env.block, now);
            self.sort_deck(env);
            ev.push(EditorEvent::ReleaseGraphics);
            self.reload_pending = true;
            ev.push(EditorEvent::Sound(sound::DECK_SWITCH));
            return;
        }
        if self.pad.repeat(pad::SQUARE | pad::CROSS) {
            if index > 0 {
                if card == EMPTY_CARD {
                    ev.push(EditorEvent::Sound(sound::REFUSED));
                } else if self.pad.repeat(pad::SQUARE) {
                    let removed =
                        slot.is_some_and(|s| remove_from_deck(env.block, env.cards, d, s));
                    if removed {
                        self.deck_list[index] = EMPTY_CARD;
                        self.sort_deck(env);
                        ev.push(EditorEvent::Animation {
                            card,
                            row: self.deck_row,
                            kind: false,
                            leader_removed: false,
                            removed: true,
                        });
                        ev.push(EditorEvent::Sound(sound::ACCEPT));
                    } else {
                        ev.push(EditorEvent::Sound(sound::REFUSED));
                    }
                } else if add_to_deck(env.block, env.cards, d, card) {
                    self.deck_list[DECK_LIST_LEN - 1] = card;
                    self.sort_deck(env);
                    ev.push(EditorEvent::Animation {
                        card,
                        row: self.deck_row,
                        kind: true,
                        leader_removed: false,
                        removed: false,
                    });
                    ev.push(EditorEvent::Sound(sound::ACCEPT));
                } else {
                    ev.push(EditorEvent::Sound(sound::REFUSED));
                }
            } else if self.pad.repeat(pad::SQUARE) {
                if self.deck_list[0] != EMPTY_CARD {
                    remove_leader(env.block, d);
                    self.deck_list[0] = EMPTY_CARD;
                    ev.push(EditorEvent::Animation {
                        card,
                        row: self.deck_row,
                        kind: false,
                        leader_removed: true,
                        removed: false,
                    });
                    ev.push(EditorEvent::ReleaseGraphics);
                    self.reload_pending = true;
                    ev.push(EditorEvent::Sound(sound::ACCEPT));
                } else {
                    ev.push(EditorEvent::Sound(sound::REFUSED));
                }
            }
            return;
        }
        if self.pad.repeat(pad::L2 | pad::R2) {
            let k = usize::from(select);
            let before = self.deck_keys[k];
            self.deck_keys[k] = cycle_key(before, self.pad.repeat(pad::L2));
            if self.deck_keys[k] != before {
                self.sort_deck(env);
                ev.push(EditorEvent::Sound(sound::SORT));
            }
            return;
        }
        if self.pad.new_press(pad::CIRCLE) {
            if self.with_detail() {
                self.state = state::DECK_LIST;
                self.slide_target = 0;
                ev.push(EditorEvent::Sound(sound::CLOSE));
            } else {
                self.try_exit(d, env, ev);
            }
            return;
        }
        if self.pad.new_press(pad::TRIANGLE) {
            if self.with_detail() {
                self.viewer(card, env, ev);
            } else {
                self.state = state::DECK_LIST_DETAIL;
                self.slide_target = -0x100;
                ev.push(EditorEvent::Sound(sound::OPEN));
            }
            return;
        }
        if self.pad.repeat(pad::LEFT | pad::RIGHT) {
            if self.pad.repeat(pad::LEFT) {
                if self.with_detail() {
                    self.state = state::DECK_LIST;
                    self.slide_target = 0;
                } else {
                    self.state = state::TRUNK_LIST;
                    self.trunk_row = self.deck_row;
                }
            } else if self.with_detail() {
                self.state = state::DECK_DETAIL;
            } else {
                self.state = state::DECK_LIST_DETAIL;
                self.slide_target = -0x100;
            }
            ev.push(EditorEvent::Sound(sound::PANE));
        }
    }

    /// State 6: the deck-card detail pane.
    fn deck_detail_state(&mut self, ev: &mut Vec<EditorEvent>) {
        if self.pad.repeat(pad::UP | pad::DOWN) {
            let before = self.deck_page;
            self.deck_page = if self.pad.repeat(pad::UP) {
                (self.deck_page + 3) % 4
            } else {
                (self.deck_page + 5) % 4
            };
            if self.deck_page != before {
                ev.push(EditorEvent::Sound(sound::SORT));
            }
            self.pad.restart(pad::LEFT | pad::RIGHT);
            return;
        }
        if self.pad.repeat(pad::LEFT) {
            self.state = state::DECK_LIST_DETAIL;
            ev.push(EditorEvent::Sound(sound::PANE));
            return;
        }
        if self.pad.new_press(pad::CIRCLE) {
            self.state = state::DECK_LIST_DETAIL;
            ev.push(EditorEvent::Sound(sound::CLOSE));
        }
    }

    /// State 7: the reincarnation yes/no prompt (LEFT = yes).
    fn reincarnation_prompt_state(&mut self, env: &mut EditorEnv<'_>, ev: &mut Vec<EditorEvent>) {
        if self.pad.new_press(pad::LEFT | pad::RIGHT) {
            let before = self.counter;
            self.counter = u8::from(self.pad.new_press(pad::LEFT));
            if self.counter != before {
                ev.push(EditorEvent::Sound(sound::CURSOR));
            }
            return;
        }
        if self.pad.new_press(pad::CROSS) {
            if self.counter != 0 {
                let card = self.trunk_card();
                for i in 0..3 {
                    self.results[i] = reincarnate_one(env.block, env.cards, card, env.rng);
                }
                delete_trunk_copy(env.block, card);
                self.sub_step = 0;
                self.counter = 0;
                self.state = state::REINCARNATION_RESULTS;
            } else {
                self.state = self.return_state;
            }
            ev.push(EditorEvent::Sound(sound::ACCEPT));
            return;
        }
        if self.pad.new_press(pad::CIRCLE) {
            self.state = self.return_state;
            ev.push(EditorEvent::Sound(sound::CLOSE));
        }
    }

    /// State 8: three timed grants, × advances, the last × resets
    /// `SAVE+0xCA`.
    fn reincarnation_results_state(&mut self, env: &mut EditorEnv<'_>, ev: &mut Vec<EditorEvent>) {
        if self.sub_step < 3 {
            if self.counter < 0x80 {
                if self.counter == REINCARNATION_GRANT_FRAME {
                    let card = self.results[usize::from(self.sub_step)];
                    if env.block.grant(card, None, env.rng) {
                        ev.push(EditorEvent::Granted(card));
                    }
                    self.rebuild_trunk(env);
                    ev.push(EditorEvent::Sound(sound::GRANT));
                }
                self.counter += 1;
            } else if self.pad.new_press(pad::CROSS) {
                ev.push(EditorEvent::Sound(sound::ACCEPT));
                self.sub_step += 1;
                if self.sub_step < 3 {
                    self.counter = 0;
                }
            }
        } else if self.pad.new_press(pad::CROSS) {
            *env.duel_counter = 0;
            ev.push(EditorEvent::DuelCounterReset);
            self.state = self.return_state;
            ev.push(EditorEvent::Sound(sound::ACCEPT));
        }
    }

    /// State 9: eight characters 0–9, a–z; UP = next, DOWN = previous.
    fn password_state(&mut self, env: &mut EditorEnv<'_>, ev: &mut Vec<EditorEvent>) {
        if self.pad.repeat(pad::LEFT | pad::RIGHT) {
            self.counter = if self.pad.repeat(pad::LEFT) {
                (self.counter + 7) % 8
            } else {
                (self.counter + 9) % 8
            };
            ev.push(EditorEvent::Sound(sound::CURSOR));
            return;
        }
        if self.pad.repeat(pad::UP | pad::DOWN) {
            let at = usize::from(self.counter & 7);
            let ch = self.password[at];
            let v = if ch < b'a' {
                ch.wrapping_sub(b'0')
            } else {
                ch.wrapping_sub(0x57)
            };
            let v = if self.pad.repeat(pad::UP) {
                (v + 37) % 36
            } else {
                (v + 35) % 36
            };
            self.password[at] = if v < 10 { b'0' + v } else { 0x57 + v };
            ev.push(EditorEvent::Sound(sound::CURSOR));
            return;
        }
        if self.pad.new_press(pad::CROSS) {
            match env.cards.find_password(&self.password) {
                None => {
                    self.state = state::MSG_PASSWORD_WRONG;
                    ev.push(EditorEvent::Sound(sound::REFUSED));
                }
                Some(card) if password_used(env.block, card) => {
                    self.state = state::MSG_PASSWORD_USED;
                    ev.push(EditorEvent::Sound(sound::ACCEPT));
                }
                Some(card) => {
                    self.results[0] = card;
                    self.state = state::MSG_PASSWORD_OK;
                    set_password_used(env.block, card, true);
                    if env.block.grant(card, None, env.rng) {
                        ev.push(EditorEvent::Granted(card));
                    }
                    self.rebuild_trunk(env);
                    ev.push(EditorEvent::Sound(sound::ACCEPT));
                }
            }
            return;
        }
        if self.pad.new_press(pad::CIRCLE) {
            self.state = self.return_state;
            ev.push(EditorEvent::Sound(sound::CLOSE));
        }
    }

    /// ○ in a list: pop with a valid current deck, else message 0xD.
    fn try_exit(&mut self, d: u8, env: &EditorEnv<'_>, ev: &mut Vec<EditorEvent>) {
        if deck_is_valid(env.block, d) {
            ev.push(EditorEvent::Sound(sound::LEAVE));
            ev.push(EditorEvent::Exit);
            self.finished = true;
        } else {
            ev.push(EditorEvent::Sound(sound::REFUSED));
            self.return_state = self.state;
            self.state = state::MSG_DECK_INVALID;
        }
    }

    /// △ with the detail pane open: the 3D viewer for a known monster.
    fn viewer(&self, card: u16, env: &EditorEnv<'_>, ev: &mut Vec<EditorEvent>) {
        let known = card < RANKED_CARDS
            && (count_copies(env.block, card, None) > 0 || seen(env.block, card));
        if known {
            ev.push(EditorEvent::Viewer(card));
            ev.push(EditorEvent::Sound(sound::ACCEPT));
        } else {
            ev.push(EditorEvent::Sound(sound::REFUSED));
        }
    }
}

/// Window deltas collected by a state for the tail (fp / s2 of 0x1B94B0).
#[derive(Clone, Copy, Debug, Default)]
struct Scroll {
    trunk: i32,
    deck: i32,
}

/// L2 = previous key, R2 = next, over 0..9.
fn cycle_key(key: u8, backwards: bool) -> u8 {
    if backwards {
        (key + 9) % 10
    } else {
        (key + 11) % 10
    }
}

/// 0x1BAEF0..: rows per frame from a stick byte: beyond the dead zone of
/// 0x30 around 0x80, `((y − 0x80 ∓ 0x30) · scale) >> 7`.
fn stick_rows(y: u8, scale: i32) -> i32 {
    let a = i32::from(y) - 0x80;
    if a.abs() < 0x31 {
        return 0;
    }
    let a = if a > 0 { a - 0x30 } else { a + 0x30 };
    (a * scale) >> 7
}

#[cfg(test)]
mod tests;
