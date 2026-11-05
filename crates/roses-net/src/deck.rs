//! Per-seat deck submission and the original's deck rules.
//!
//! Each seat submits its own deck in its handshake ([`crate::ClientMessage::Hello`]);
//! the server checks it with [`check_deck`] and refuses an illegal deck
//! before the match begins. A seat's deck is never sent to the other seat
//! (the server keeps it in its own match record only).
//!
//! The rules are the ones the original enforces on a deck that reaches a
//! duel (research: `modes/the project notes, `modes/custom-duel/NOTES.md`):
//! - Custom Duel's Select Deck commits a deck record only with 40 cards and
//!   a leader (0x1BB0A0, `roses_core::deck_editor::DeckRecord::is_valid`);
//! - the editor never puts a fourth copy of one card into a deck (0x214C70,
//!   [`roses_core::deck_editor::MAX_COPIES_IN_DECK`]; the leader is a
//!   separate slot and does not count);
//! - every card needs a static card record (ids below 854, not 0x29F) and,
//!   for the online duel, an entry in the shared rules catalogue;
//! - a leader is set from a trunk card whose stored rank is above 0 (Start
//!   in the editor's trunk list, states 2/3 of `roses_core::deck_editor`):
//!   a ranked id (below 0x2AB, not 0x29F) with rank 1..=12.
//!
//! No deck-cost (DC) limit applies: in the original the DC check only gates
//! the campaign map's duels (0x239810, opponent deck cost against the
//! player's); Custom Duel never reads it. Card ownership cannot be verified
//! without accounts: a seat may submit any deck that follows these rules.

use roses_core::card_block::{CardBlock, EMPTY_CARD};
use roses_core::deck_editor::{self, DECKS_PER_BLOCK, DECK_SIZE, MAX_COPIES_IN_DECK};
use roses_core::duel::{CardData, RANKED_CARDS, RANK_COUNT, REJECTED_DECK_CARD};
use roses_core::CARD_COUNT;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A seat's deck as it submits it: the deck record's cards in stored order
/// (the server shuffles them at match start with its secret seed), the
/// leader and the leader's stored rank (its insignia in the HUD).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckSubmission {
    pub leader: u16,
    /// The leader's stored rank (`CR+0x48 & 0x3FFF`), 1..=12.
    pub leader_rank: u8,
    pub cards: Vec<u16>,
}

/// Why a submitted deck is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeckProblem {
    /// Not exactly 40 cards.
    WrongSize { count: usize },
    /// No leader (999).
    NoLeader,
    /// A card without a static record or not in the rules catalogue.
    UnknownCard { card: u16 },
    /// More than three copies of one card.
    TooManyCopies { card: u16, count: usize },
    /// The leader is not a ranked card.
    LeaderNotRanked { leader: u16 },
    /// The leader's rank is outside 1..=12.
    BadLeaderRank { rank: u8 },
    /// The duel could not be set up with the decks (the engine's reason).
    Unplayable { reason: String },
}

impl std::fmt::Display for DeckProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongSize { count } => write!(f, "a deck needs {DECK_SIZE} cards, this one has {count}"),
            Self::NoLeader => f.write_str("the deck has no leader"),
            Self::UnknownCard { card } => write!(f, "card {card} is not a card the rules know"),
            Self::TooManyCopies { card, count } => {
                write!(f, "card {card} appears {count} times (at most {MAX_COPIES_IN_DECK})")
            }
            Self::LeaderNotRanked { leader } => write!(f, "card {leader} cannot lead a deck (no rank)"),
            Self::BadLeaderRank { rank } => write!(f, "leader rank {rank} is outside 1..=12"),
            Self::Unplayable { reason } => write!(f, "the duel cannot start with this deck: {reason}"),
        }
    }
}

fn has_record(card: u16) -> bool {
    card < CARD_COUNT && card != REJECTED_DECK_CARD
}

/// The original's deck rules (see the module notes); `cards` is the rules
/// catalogue the match runs with.
pub fn check_deck(deck: &DeckSubmission, cards: &impl CardData) -> Result<(), DeckProblem> {
    if deck.cards.len() != DECK_SIZE {
        return Err(DeckProblem::WrongSize {
            count: deck.cards.len(),
        });
    }
    if deck.leader == EMPTY_CARD {
        return Err(DeckProblem::NoLeader);
    }
    let known = |c: u16| has_record(c) && cards.card(c).is_some();
    if !known(deck.leader) {
        return Err(DeckProblem::UnknownCard { card: deck.leader });
    }
    if deck.leader >= RANKED_CARDS {
        return Err(DeckProblem::LeaderNotRanked {
            leader: deck.leader,
        });
    }
    if deck.leader_rank == 0 || usize::from(deck.leader_rank) >= RANK_COUNT {
        return Err(DeckProblem::BadLeaderRank {
            rank: deck.leader_rank,
        });
    }
    let mut copies: BTreeMap<u16, usize> = BTreeMap::new();
    for &card in &deck.cards {
        if !known(card) {
            return Err(DeckProblem::UnknownCard { card });
        }
        *copies.entry(card).or_default() += 1;
    }
    if let Some((&card, &count)) = copies.iter().find(|(_, &n)| n > MAX_COPIES_IN_DECK) {
        return Err(DeckProblem::TooManyCopies { card, count });
    }
    Ok(())
}

/// Deck record `deck` (0..=2; `None` = the block's current deck) of a card
/// block, as the duel setup reads it (0x214120: the record's cards in slot
/// order, 999s skipped; the leader's stored rank).
pub fn from_card_block(block: &CardBlock, deck: Option<u8>) -> Result<DeckSubmission, String> {
    let d = deck.unwrap_or_else(|| deck_editor::current_deck(block));
    if d >= DECKS_PER_BLOCK {
        return Err(format!("deck {d} is not one of the three decks"));
    }
    let record = deck_editor::deck_record(block, d);
    if !record.is_valid() {
        return Err(format!(
            "deck {d} is not valid ({} cards, leader {})",
            record.count(),
            record.leader
        ));
    }
    let rank = block.stored_rank(record.leader).unwrap_or(0);
    Ok(DeckSubmission {
        leader: record.leader,
        leader_rank: u8::try_from(rank).unwrap_or(u8::MAX),
        cards: record.cards.iter().copied().filter(|&c| c != EMPTY_CARD).collect(),
    })
}

/// The save's data file: `path` itself, or `<path>/memory-card/slot1/...`,
/// or `<path>/BESLES-51579/BESLES-51579` (a slot folder).
pub fn find_save_file(path: &Path) -> PathBuf {
    use roses_core::memory_card::PAL_NAME;
    if path.is_file() {
        return path.to_path_buf();
    }
    let in_slot = |slot: &Path| slot.join(PAL_NAME).join(PAL_NAME);
    let candidates = [
        in_slot(path),
        in_slot(&path.join("memory-card").join("slot1")),
        in_slot(&path.join("slot1")),
    ];
    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .unwrap_or_else(|| in_slot(path))
}

/// A deck from the user's own memory-card data file (the original's
/// `BESLES-51579` file, checksum verified): its card block's deck record.
pub fn from_save_file(path: &Path, deck: Option<u8>) -> Result<DeckSubmission, String> {
    use roses_core::memory_card::{SaveFile, FILE_BYTES};
    let file = find_save_file(path);
    let meta = std::fs::metadata(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    if meta.len() > FILE_BYTES as u64 * 2 {
        return Err(format!("{} is not a memory-card data file", file.display()));
    }
    let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let parsed = SaveFile::parse(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    if !parsed.checksum_ok {
        return Err(format!("{}: the checksum does not match", file.display()));
    }
    let block = CardBlock::from_bytes(&parsed.file.block).map_err(|e| format!("card block: {e:?}"))?;
    from_card_block(&block, deck)
}

/// A deck from a JSON file holding a [`DeckSubmission`].
pub fn from_json_file(path: &Path) -> Result<DeckSubmission, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > 64 * 1024 {
        return Err(format!("{} is too large for a deck", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// A side of the import's first duel as a submission (the first duel's
/// decks are the player's starter deck and Weevil's; the import does not
/// carry their ranks, so the leader is submitted at rank 1).
pub fn from_import_side(side: &crate::data::ImportSide) -> DeckSubmission {
    DeckSubmission {
        leader: side.leader,
        leader_rank: 1,
        cards: side.cards.clone(),
    }
}

// ---- The game's own opponent decks -----------------------------------------

/// The first static deck record of an opponent: opponent id `o` (the
/// campaign's controller byte, speaker message − 197) plays record
/// `0x1A + o` (0x214120(1, 0x1A + C4); Custom Duel's computer opponents are
/// records 0x30..=0x32, ids 0x16..=0x18) [F NEXT-DECKS.md §1].
pub const OPPONENT_FIRST_RECORD: u8 = 0x1A;
/// Opponent ids 0..=0x18 (records 0x1A..=0x32, the last of the 51).
pub const OPPONENT_COUNT: u8 = 25;
/// MSG_E.DAT message of opponent 0's name (speaker 197 + id).
pub const OPPONENT_NAME_MESSAGE: u16 = 197;

/// One computer opponent's static deck as the game defines it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpponentDeck {
    /// The opponent id (controller byte).
    pub opponent: u8,
    /// The static deck record (0x1A + id).
    pub record: u8,
    /// The opponent's name from the user's MSG_E.DAT (message 197 + id),
    /// else "OPPONENT n".
    pub name: String,
    /// The record's 40 cards, its leader and the leader's rank (the high
    /// nibble of the leader word, the rank the game gives the leader).
    pub deck: DeckSubmission,
}

/// A static deck record as a submission (the leader word's rank nibble is
/// the leader's rank; cards `& 0xFFF` as the loader reads them).
pub fn from_static_record(record: &roses_assets::DeckRecord) -> DeckSubmission {
    DeckSubmission {
        leader: record.leader_card,
        leader_rank: record.leader_rank_raw,
        cards: record.cards.clone(),
    }
}

/// The opponents' decks from the executable's deck table (51 records) and,
/// when given, their names from MSG_E.DAT.
pub fn opponent_decks_from(records: &[roses_assets::DeckRecord], names: &dyn Fn(u8) -> Option<String>) -> Vec<OpponentDeck> {
    (0..OPPONENT_COUNT)
        .filter_map(|o| {
            let record = OPPONENT_FIRST_RECORD + o;
            let r = records.iter().find(|r| r.index == record)?;
            Some(OpponentDeck {
                opponent: o,
                record,
                name: names(o).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("OPPONENT {o}")),
                deck: from_static_record(r),
            })
        })
        .collect()
}

/// The text of MSG_E.DAT messages (glyphs with a known character; the
/// space code 0x2001 as a space; anything else dropped).
pub fn message_texts(messages: &[u8], ids: &[u16]) -> Result<Vec<String>, String> {
    use roses_assets::MessageToken;
    let parsed = roses_assets::parse_pal_messages(messages, ids).map_err(|e| e.to_string())?;
    Ok(parsed
        .messages
        .iter()
        .map(|m| {
            let mut s = String::new();
            for line in &m.lines {
                if !s.is_empty() {
                    s.push(' ');
                }
                for t in &line.tokens {
                    match t {
                        MessageToken::Glyph { character: Some(c), .. } => s.push(*c),
                        MessageToken::Glyph { code: 0x2001, .. } | MessageToken::Unknown { code: 0x2001 } => s.push(' '),
                        _ => {}
                    }
                }
            }
            s.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect())
}

/// The opponents' decks of the user's own executable, named from their
/// MSG_E.DAT (`messages`: `None` = numbered names).
pub fn opponent_decks(executable: &[u8], messages: Option<&[u8]>) -> Result<Vec<OpponentDeck>, String> {
    let table = roses_assets::parse_pal_decks(executable).map_err(|e| e.to_string())?;
    let ids: Vec<u16> = (0..OPPONENT_COUNT).map(opponent_name_message).collect();
    let names = match messages {
        Some(m) => message_texts(m, &ids).unwrap_or_default(),
        None => Vec::new(),
    };
    let mut decks = opponent_decks_from(&table.decks, &|o| names.get(usize::from(o)).cloned());
    // Two forms of one duelist (Manawyddan fab Llyr): numbered.
    for i in 0..decks.len() {
        let same = decks[..i].iter().filter(|d| d.name == decks[i].name || d.name.starts_with(&format!("{} ", decks[i].name))).count();
        if same > 0 {
            decks[i].name = format!("{} {}", decks[i].name, same + 1);
        }
    }
    Ok(decks)
}

/// The message naming opponent `o`: the speaker's name (197 + id); for
/// Custom Duel's computer opponents (records 0x30..=0x32, Select Deck's
/// selectors 4..2) the plate Custom Duel draws over side 1 (0x8D +
/// selector: "Deck Master S/T/I") [F 0x1C8C00].
pub fn opponent_name_message(o: u8) -> u16 {
    match o {
        0x16..=0x18 => 141 + u16::from(0x1A - o),
        _ => OPPONENT_NAME_MESSAGE + u16::from(o),
    }
}

/// Whether opponent `o` is one of Custom Duel's computer opponents.
pub fn is_custom_duel_opponent(o: u8) -> bool {
    (0x16..=0x18).contains(&o)
}

/// The opponents' decks of a duel import folder (its SLES_515.79 and
/// MSG_E.DAT).
pub fn load_opponent_decks(duel_dir: &Path) -> Result<Vec<OpponentDeck>, String> {
    let dir = if duel_dir.join("SLES_515.79").is_file() { duel_dir.to_path_buf() } else { duel_dir.join("duel") };
    let exe = std::fs::read(dir.join("SLES_515.79")).map_err(|e| format!("{}: {e}", dir.join("SLES_515.79").display()))?;
    let messages = std::fs::read(dir.join("MSG_E.DAT")).ok();
    opponent_decks(&exe, messages.as_deref())
}
