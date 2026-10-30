//! Fingerprinted static deck table (51 records) from the PAL executable.
//!
//! Loader 0x214120 indexes `0x2D10E0 + index * 82` for indices 0..=0x32 and
//! reads 41 halfwords: word 0 is the leader (`rank << 12 | card`, split at
//! 0x2142B0/0x2142B8) and words 1..=40 are the deck cards (`& 0xFFF`,
//! 0x214260..0x21427C). Indices below 0x11 take an extra randomized path in
//! the loader; that behaviour is not interpreted here. These are data records,
//! not deck-building or shuffle rules.

use super::{require_pal_executable, Error, Result, CARD_COUNT, PROFILE_ID};
use serde::Serialize;

pub const DECK_COUNT: usize = 51;
pub const DECK_CARDS: usize = 40;
pub const DECK_RECORD_STRIDE: usize = 82;
pub const DECK_TABLE_EE_ADDRESS: u32 = 0x002D_10E0;
pub const DECK_TABLE_FILE_OFFSET: usize = 0x1D_7A60;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DeckRecord {
    pub index: u8,
    pub leader_card: u16,
    /// High nibble of the leader word; its gameplay meaning (leader rank) is
    /// not validated here.
    pub leader_rank_raw: u8,
    pub cards: Vec<u16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeckTable {
    pub schema_version: u32,
    pub profile_id: String,
    pub executable_sha256: String,
    pub table_ee_address: u32,
    pub decks: Vec<DeckRecord>,
    pub gameplay_validated: bool,
}

/// Import the supported PAL executable's 51 static deck records.
pub fn parse_pal_decks(executable: &[u8]) -> Result<DeckTable> {
    let executable_sha256 = require_pal_executable(executable)?;
    let end = DECK_TABLE_FILE_OFFSET + DECK_COUNT * DECK_RECORD_STRIDE;
    let bytes = executable
        .get(DECK_TABLE_FILE_OFFSET..end)
        .ok_or_else(|| Error::Invalid("deck table lies outside executable".into()))?;
    Ok(DeckTable {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        executable_sha256,
        table_ee_address: DECK_TABLE_EE_ADDRESS,
        decks: parse_deck_records(bytes)?,
        gameplay_validated: false,
    })
}

fn parse_deck_records(bytes: &[u8]) -> Result<Vec<DeckRecord>> {
    if bytes.is_empty()
        || !bytes.len().is_multiple_of(DECK_RECORD_STRIDE)
        || bytes.len() / DECK_RECORD_STRIDE > DECK_COUNT
    {
        return Err(Error::Invalid("deck table has an invalid length".into()));
    }
    bytes
        .chunks_exact(DECK_RECORD_STRIDE)
        .enumerate()
        .map(|(index, record)| {
            let word = |i: usize| u16::from_le_bytes([record[2 * i], record[2 * i + 1]]);
            let leader = word(0);
            let leader_card = leader & 0x0FFF;
            let cards: Vec<u16> = (1..=DECK_CARDS).map(word).collect();
            if usize::from(leader_card) >= CARD_COUNT
                || leader >> 12 == 0
                || cards.iter().any(|&c| usize::from(c) >= CARD_COUNT)
            {
                return Err(Error::Invalid(format!(
                    "deck {index} has an out-of-range leader or card"
                )));
            }
            Ok(DeckRecord {
                index: index as u8,
                leader_card,
                leader_rank_raw: (leader >> 12) as u8,
                cards,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(leader: u16, cards: [u16; DECK_CARDS]) -> Vec<u8> {
        std::iter::once(leader)
            .chain(cards)
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    #[test]
    fn leader_word_splits_rank_and_card_and_cards_are_bounded() {
        let mut bytes = record(0x103C, [156; DECK_CARDS]);
        bytes.extend(record(0xC000, [853; DECK_CARDS]));
        let decks = parse_deck_records(&bytes).unwrap();
        assert_eq!((decks[0].leader_card, decks[0].leader_rank_raw), (60, 1));
        assert_eq!((decks[1].leader_card, decks[1].leader_rank_raw), (0, 12));
        assert_eq!(decks[1].cards.len(), DECK_CARDS);
        assert!(parse_deck_records(&record(0x1000 | 854, [0; DECK_CARDS])).is_err());
        assert!(parse_deck_records(&record(0x1000, [854; DECK_CARDS])).is_err());
        assert!(
            parse_deck_records(&record(60, [0; DECK_CARDS])).is_err(),
            "rank 0"
        );
        assert!(parse_deck_records(&bytes[..81]).is_err());
    }
}
