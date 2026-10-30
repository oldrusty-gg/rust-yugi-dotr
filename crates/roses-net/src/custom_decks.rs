//! The player's own custom decks for online play (a port addition): any
//! card of the game, no ownership limits, kept in `custom-decks.json` in
//! the game data folder (beside `online-session.json`), never in the
//! memory-card save. A versioned schema; unknown fields are refused so a
//! newer file is not silently rewritten by an older game.
//!
//! The deck builder edits a custom deck with the game's own deck editor
//! ([`roses_core::deck_editor`]) on a *full-pool* card block
//! ([`editor_block`]): every card with a static record is in the trunk
//! (three copies, four for a card that can lead, so a leader may sit beside
//! three copies of its own card as the deck rules allow), every card is
//! seen, and every ranked card has the deck's leader rank. The deck is
//! read back from deck record 0 ([`from_editor_block`]).
//!
//! Legality is the server's ([`crate::deck::check_deck`]): 40 cards, at
//! most three copies of a card, a leader that is a ranked card, leader rank
//! 1..=12. [`CustomDeck::problem`] checks the same rules without the rules
//! catalogue so the screens can say what is missing before a match.

use crate::deck::DeckSubmission;
use roses_core::card_block::{CardBlock, EMPTY_CARD, OWNED_BIT, RANK_MASK};
use roses_core::deck_editor::{self, StaticCards, DECK_SIZE, MAX_COPIES_IN_DECK};
use roses_core::duel::{RANKED_CARDS, REJECTED_DECK_CARD};
use roses_core::CARD_COUNT;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const CUSTOM_DECKS_FILE: &str = "custom-decks.json";
pub const CUSTOM_DECKS_SCHEMA: &str = "roses-custom-decks-1";
/// The longest deck name (the screens squeeze a long name into its row;
/// 24 holds an opponent's name with " (copy)", "Pegasus Crawford (copy)").
pub const NAME_MAX: usize = 24;
/// What a copied deck's name ends with.
pub const COPY_SUFFIX: &str = " (copy)";
/// Decks a file may hold.
pub const MAX_DECKS: usize = 99;
/// Leader ranks a deck may choose (the original's ranks 1..=12, NCO..SD;
/// rank 0 never leads).
pub const RANK_MIN: u8 = 1;
pub const RANK_MAX: u8 = 12;
/// The rank a new deck's leader starts at.
pub const DEFAULT_RANK: u8 = 1;
/// The largest file read.
const FILE_MAX: u64 = 1024 * 1024;

/// One custom deck.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomDeck {
    pub name: String,
    /// The leader card, 999 = none yet.
    pub leader: u16,
    pub leader_rank: u8,
    /// The deck's cards (0..=40 of them while it is being built).
    pub cards: Vec<u16>,
}

/// A character a deck name may hold: what the game's message font draws.
pub fn name_char_ok(c: char) -> bool {
    c.is_ascii_alphanumeric() || " -'.!?&()#".contains(c)
}

/// A typed name tidied: allowed characters, single spaces, at most
/// [`NAME_MAX`] characters.
pub fn clean_name(name: &str) -> String {
    let kept: String = name.chars().filter(|&c| name_char_ok(c)).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(NAME_MAX).collect()
}

/// Whether a card has a static record (ids below 854, not 0x29F).
pub fn card_ok(card: u16) -> bool {
    card < CARD_COUNT && card != REJECTED_DECK_CARD
}

/// Whether a card can lead a deck (a ranked id).
pub fn can_lead(card: u16) -> bool {
    card_ok(card) && card < RANKED_CARDS
}

impl CustomDeck {
    pub fn new(name: &str) -> Self {
        Self { name: clean_name(name), leader: EMPTY_CARD, leader_rank: DEFAULT_RANK, cards: Vec::new() }
    }

    /// The first rule the deck breaks (the server's rules, without the
    /// catalogue), `None` when it is ready for online play.
    pub fn problem(&self) -> Option<String> {
        if self.cards.len() != DECK_SIZE {
            return Some(format!("{} of {DECK_SIZE} cards", self.cards.len()));
        }
        if self.leader == EMPTY_CARD {
            return Some("no leader".into());
        }
        if !can_lead(self.leader) {
            return Some(format!("card {} cannot lead", self.leader));
        }
        if !(RANK_MIN..=RANK_MAX).contains(&self.leader_rank) {
            return Some(format!("leader rank {} is outside {RANK_MIN}..{RANK_MAX}", self.leader_rank));
        }
        if let Some(&bad) = self.cards.iter().find(|&&c| !card_ok(c)) {
            return Some(format!("card {bad} is not a card"));
        }
        let mut sorted = self.cards.clone();
        sorted.sort_unstable();
        for w in sorted.chunk_by(|a, b| a == b) {
            if w.len() > MAX_COPIES_IN_DECK {
                return Some(format!("{} copies of card {}", w.len(), w[0]));
            }
        }
        None
    }

    pub fn is_complete(&self) -> bool {
        self.problem().is_none()
    }

    /// The deck as it is submitted at join.
    pub fn submission(&self) -> Result<DeckSubmission, String> {
        match self.problem() {
            Some(p) => Err(p),
            None => Ok(DeckSubmission { leader: self.leader, leader_rank: self.leader_rank, cards: self.cards.clone() }),
        }
    }

    /// A custom deck from any deck (a copy of a save or opponent deck).
    pub fn from_submission(name: &str, deck: &DeckSubmission) -> Self {
        Self {
            name: clean_name(name),
            leader: deck.leader,
            leader_rank: deck.leader_rank.clamp(RANK_MIN, RANK_MAX),
            cards: deck.cards.iter().copied().filter(|&c| card_ok(c)).take(DECK_SIZE).collect(),
        }
    }
}

/// The file's content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomDecks {
    pub schema: String,
    pub decks: Vec<CustomDeck>,
}

impl Default for CustomDecks {
    fn default() -> Self {
        Self { schema: CUSTOM_DECKS_SCHEMA.into(), decks: Vec::new() }
    }
}

impl CustomDecks {
    /// The decks at `path`; no file = none yet.
    pub fn load(path: &Path) -> Result<Self, String> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        if meta.len() > FILE_MAX {
            return Err(format!("{} is too large", path.display()));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let decks: Self = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if decks.schema != CUSTOM_DECKS_SCHEMA {
            return Err(format!("{}: unsupported schema {} (a newer game wrote it?)", path.display(), decks.schema));
        }
        Ok(decks)
    }

    /// Write the file (a temporary file renamed over it).
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(io::Error::other)?)?;
        std::fs::rename(&tmp, path)
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.decks.iter().position(|d| d.name.eq_ignore_ascii_case(name))
    }

    /// `base` (tidied, "DECK" when empty), numbered when a deck has that
    /// name already ("DECK 2", ...); `except` is the deck being renamed.
    pub fn unique_name(&self, base: &str, except: Option<usize>) -> String {
        let base = match clean_name(base) {
            b if b.is_empty() => "DECK".to_string(),
            b => b,
        };
        let taken = |n: &str| self.decks.iter().enumerate().any(|(i, d)| Some(i) != except && d.name.eq_ignore_ascii_case(n));
        if !taken(&base) {
            return base;
        }
        (2..)
            .map(|k| {
                let suffix = format!(" {k}");
                let stem: String = base.chars().take(NAME_MAX - suffix.len()).collect();
                format!("{}{suffix}", stem.trim_end())
            })
            .find(|n| !taken(n))
            .expect("a free number")
    }

    /// The name of a copy of the deck called `base`: "BASE (copy)", then
    /// "BASE (copy 2)", ... (the base shortened to fit [`NAME_MAX`]).
    pub fn copy_name(&self, base: &str) -> String {
        let base = match clean_name(base) {
            b if b.is_empty() => "DECK".to_string(),
            b => b,
        };
        let taken = |n: &str| self.decks.iter().any(|d| d.name.eq_ignore_ascii_case(n));
        (1..)
            .map(|k| {
                let suffix = if k == 1 { COPY_SUFFIX.to_string() } else { format!(" (copy {k})") };
                let stem: String = base.chars().take(NAME_MAX - suffix.len()).collect();
                format!("{}{suffix}", stem.trim_end())
            })
            .find(|n| !taken(n))
            .expect("a free number")
    }

    /// Add a deck under a unique name; its index (`None` when full).
    pub fn add(&mut self, mut deck: CustomDeck) -> Option<usize> {
        if self.decks.len() >= MAX_DECKS {
            return None;
        }
        deck.name = self.unique_name(&deck.name, None);
        self.decks.push(deck);
        Some(self.decks.len() - 1)
    }

    /// A copy of deck `i` right after it ("NAME 2").
    pub fn duplicate(&mut self, i: usize) -> Option<usize> {
        let deck = self.decks.get(i)?.clone();
        if self.decks.len() >= MAX_DECKS {
            return None;
        }
        let name = self.unique_name(&deck.name, None);
        self.decks.insert(i + 1, CustomDeck { name, ..deck });
        Some(i + 1)
    }

    /// Rename deck `i` (tidied, made unique); the name it got.
    pub fn rename(&mut self, i: usize, name: &str) -> Option<String> {
        let name = self.unique_name(name, Some(i));
        self.decks.get_mut(i)?.name = name.clone();
        Some(name)
    }

    pub fn delete(&mut self, i: usize) -> Option<CustomDeck> {
        (i < self.decks.len()).then(|| self.decks.remove(i))
    }
}

/// Owner key of the full-pool block's copies (any non-zero key).
const POOL_KEY: u32 = 0x524F_5345;

/// The deck editor's block for a custom deck: every card in the trunk
/// (3 copies, 4 for a card that can lead), all seen, every ranked card at
/// the deck's leader rank, the deck laid out in record 0 (cards in order,
/// the leader flagged) and record 0 current. Cards the pool cannot give
/// (not a card, a fourth copy) are dropped.
pub fn editor_block(deck: &CustomDeck, cards: &StaticCards) -> CardBlock {
    let mut block = CardBlock::empty(POOL_KEY);
    let mut rng = 0x5EED_u64;
    let rank = u16::from(deck.leader_rank.clamp(RANK_MIN, RANK_MAX));
    for card in 0..CARD_COUNT {
        if !card_ok(card) {
            continue;
        }
        let copies = if can_lead(card) { MAX_COPIES_IN_DECK + 1 } else { MAX_COPIES_IN_DECK };
        for _ in 0..copies {
            block.grant(card, None, &mut rng);
        }
        if let Some(r) = block.records.get_mut(usize::from(card)) {
            r.rank_word |= OWNED_BIT;
            if card < RANKED_CARDS {
                r.rank_word = (r.rank_word & !RANK_MASK) | rank;
            }
        }
    }
    // The recent list as a new block has it (the grants filled it).
    block.recent = [EMPTY_CARD; roses_core::card_block::RECENT_LEN];
    let mut kept: Vec<u16> = Vec::new();
    for &c in &deck.cards {
        let n = kept.iter().filter(|&&k| k == c).count();
        if card_ok(c) && n < MAX_COPIES_IN_DECK && kept.len() < DECK_SIZE {
            kept.push(c);
        }
    }
    let leader = if can_lead(deck.leader) { deck.leader } else { EMPTY_CARD };
    if deck_editor::lay_out_new_game_deck(&mut block, cards, 0, &kept, leader).is_err() {
        // No leader (or none possible): lay out the cards alone.
        let _ = deck_editor::lay_out_new_game_deck(&mut block, cards, 0, &kept, 0);
        let _ = deck_editor::remove_leader(&mut block, 0);
    }
    block
}

/// The deck the editor left in `block` (the current deck record, as the
/// online menu builds one deck at a time): its cards in slot order and its
/// leader; the name and rank stay the custom deck's.
pub fn from_editor_block(block: &CardBlock, deck: &CustomDeck) -> CustomDeck {
    let record = deck_editor::deck_record(block, deck_editor::current_deck(block));
    CustomDeck {
        name: deck.name.clone(),
        leader: record.leader,
        leader_rank: deck.leader_rank,
        cards: record.cards.iter().copied().filter(|&c| c != EMPTY_CARD).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn static_cards() -> StaticCards {
        // Cost = id % 7 for every card; kinds as the records leave them.
        let records = (0..CARD_COUNT)
            .map(|c| {
                let mut r = [0u8; deck_editor::RECORD_BYTES];
                r[3] = (c % 7) as u8;
                r
            })
            .collect();
        StaticCards { records, name_order: (0..CARD_COUNT).collect(), password_key: [0; 8] }
    }

    fn full_deck() -> CustomDeck {
        let mut cards: Vec<u16> = (0..13).flat_map(|c| [c, c, c]).collect();
        cards.push(700);
        CustomDeck { name: "Dragons".into(), leader: 1, leader_rank: 5, cards }
    }

    #[test]
    fn the_rules_are_the_servers() {
        let d = full_deck();
        assert_eq!(d.problem(), None);
        assert_eq!(d.submission().unwrap().cards, d.cards);
        let mut d2 = d.clone();
        d2.cards.pop();
        assert_eq!(d2.problem(), Some("39 of 40 cards".into()));
        let mut d2 = d.clone();
        d2.leader = EMPTY_CARD;
        assert_eq!(d2.problem(), Some("no leader".into()));
        let mut d2 = d.clone();
        d2.leader = 700;
        assert!(d2.problem().unwrap().contains("cannot lead"));
        for rank in [0, 13] {
            let mut d2 = d.clone();
            d2.leader_rank = rank;
            assert!(d2.problem().unwrap().contains("rank"));
        }
        let mut d2 = d.clone();
        d2.cards[39] = 0;
        assert_eq!(d2.problem(), Some("4 copies of card 0".into()));
        let mut d2 = d.clone();
        d2.cards[39] = REJECTED_DECK_CARD;
        assert!(d2.problem().unwrap().contains("not a card"));
    }

    #[test]
    fn names_are_tidied_and_unique() {
        let mut all = CustomDecks::default();
        assert_eq!(clean_name("  My   ~deck!!  with a very long name "), "My deck!! with a very lo");
        assert_eq!(all.add(CustomDeck::new("Dragons")), Some(0));
        assert_eq!(all.add(CustomDeck::new("dragons")), Some(1));
        assert_eq!(all.decks[1].name, "dragons 2");
        assert_eq!(all.duplicate(0), Some(1));
        assert_eq!(all.decks[1].name, "Dragons 3");
        assert_eq!(all.rename(2, "Dragons"), Some("Dragons 2".into()));
        assert_eq!(all.rename(0, "Dragons"), Some("Dragons".into()), "its own name is free");
        assert_eq!(all.unique_name("", None), "DECK");
        let long = "ABCDEFGHIJKLMNOPQRSTUVWX";
        assert_eq!(all.unique_name(long, None), long);
        all.add(CustomDeck::new(long));
        assert_eq!(all.unique_name(long, None), "ABCDEFGHIJKLMNOPQRSTUV 2");
        // Copies: "(copy)", then numbered; a long base is shortened.
        assert_eq!(all.copy_name("Pegasus Crawford"), "Pegasus Crawford (copy)");
        all.add(CustomDeck::new("pegasus crawford (copy)"));
        assert_eq!(all.copy_name("Pegasus Crawford"), "Pegasus Crawfor (copy 2)");
        assert_eq!(all.copy_name("Christian Rosenkreuz"), "Christian Rosenkr (copy)");
        assert_eq!(all.delete(0).unwrap().name, "Dragons");
        assert_eq!(all.delete(9), None);
    }

    #[test]
    fn the_file_round_trips_and_refuses_other_schemas() {
        let dir = std::env::temp_dir().join(format!("roses-custom-decks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(CUSTOM_DECKS_FILE);
        assert_eq!(CustomDecks::load(&path).unwrap(), CustomDecks::default());
        let mut all = CustomDecks::default();
        all.add(full_deck());
        all.save(&path).unwrap();
        assert_eq!(CustomDecks::load(&path).unwrap(), all);
        std::fs::write(&path, br#"{"schema":"roses-custom-decks-9","decks":[]}"#).unwrap();
        assert!(CustomDecks::load(&path).unwrap_err().contains("unsupported schema"));
        std::fs::write(&path, br#"{"schema":"roses-custom-decks-1","decks":[],"extra":1}"#).unwrap();
        assert!(CustomDecks::load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The game's own deck editor on the full-pool block: the deck comes
    /// back unchanged; every card can be added up to three copies; a
    /// leader can join three copies of its own card.
    #[test]
    fn the_full_pool_block_carries_the_deck_through_the_editor_rules() {
        let cards = static_cards();
        let deck = full_deck();
        let mut block = editor_block(&deck, &cards);
        let back = from_editor_block(&block, &deck);
        assert_eq!(back, deck);
        assert_eq!(block.stored_rank(5), Some(5));
        assert_eq!(block.stored_rank(700), None);
        assert!(deck_editor::seen(&block, 853));
        // Remove a copy of card 0 and add card 852 (never owned before).
        let slot = deck_editor::deck_record(&block, 0).index_of(0).unwrap();
        assert!(deck_editor::remove_from_deck(&mut block, &cards, 0, slot));
        assert!(deck_editor::add_to_deck(&mut block, &cards, 0, 852));
        let back = from_editor_block(&block, &deck);
        assert_eq!(back.problem(), None);
        assert!(back.cards.contains(&852));
        // An empty deck: three copies of a card, then the same card as leader.
        let empty = CustomDeck::new("New");
        let mut block = editor_block(&empty, &cards);
        assert_eq!(deck_editor::deck_record(&block, 0).leader, EMPTY_CARD);
        assert_eq!(deck_editor::deck_record(&block, 0).count(), 0);
        for _ in 0..3 {
            assert!(deck_editor::add_to_deck(&mut block, &cards, 0, 42));
        }
        assert!(!deck_editor::add_to_deck(&mut block, &cards, 0, 42), "a fourth copy is refused");
        assert!(deck_editor::set_leader(&mut block, 0, 42));
        assert!(!deck_editor::add_to_deck(&mut block, &cards, 0, REJECTED_DECK_CARD));
        let back = from_editor_block(&block, &empty);
        assert_eq!((back.leader, back.cards.len()), (42, 3));
    }
}
