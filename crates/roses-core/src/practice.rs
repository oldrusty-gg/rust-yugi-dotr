//! Opening practice-duel setup, from traced source plus saved-state checks.
//!
//! Source (PAL): practice setup 0x23B590 reads the practice counter byte
//! 0x312B80 (values 0..=4) and loads deck-table records (0x11,0x12),
//! (0x13,0x14), (0x15,0x16), (0x17,0x18) or (0x19,0x1A) for sides 0 and 1
//! through loader 0x214120. Afterwards it advances the counter, wrapping
//! values that reach 3 back to 0. Duel initialization stores 4000 LP per side
//! (0x1AEA1C, 0x1AEAC0).
//!
//! Observed, not decoded (saved original states, see private
//! the project notes): for pair 0 the leaders stand at column 3,
//! rows 6 (side 0) and 0 (side 1); every cell has terrain code 7 (map 0); the
//! tutorial's first hand is the first five cards of side 0's record in table
//! order (no shuffle observed for this pair). Other pairs, shuffling in other
//! modes, draw rules and turn order are not established here.
use crate::{
    CoreState, GridPos, StateDocument, StateError, Unit, UnitId, CELL_COUNT, STATE_SCHEMA,
};
use serde::{Deserialize, Serialize};

pub const PRACTICE_PAIRS: [(u8, u8); 5] = [
    (0x11, 0x12),
    (0x13, 0x14),
    (0x15, 0x16),
    (0x17, 0x18),
    (0x19, 0x1A),
];
pub const STARTING_LIFE_POINTS: u16 = 4000;
pub const OBSERVED_HAND_SIZE: usize = 5;
pub const DECK_SIZE: usize = 40;

/// A deck record as imported from the user's executable (see roses-assets).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckInput {
    pub index: u8,
    pub leader_card: u16,
    pub leader_rank_raw: u8,
    pub cards: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideSetup {
    pub deck_index: u8,
    pub leader_rank_raw: u8,
    /// Remaining draw pile, next card first.
    pub draw_pile: Vec<u16>,
    pub hand: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PracticeStart {
    pub counter: u8,
    pub board: CoreState,
    pub sides: [SideSetup; 2],
    /// Scope of this start: which parts are verified for this pair.
    pub verified_against_original: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PracticeError {
    Counter(u8),
    MissingDeck(u8),
    DeckSize { index: u8, cards: usize },
    State(StateError),
}

/// Leader cells observed for pair 0 (column 3, rows 6 and 0).
const LEADER_CELLS: [GridPos; 2] = [GridPos { column: 3, row: 6 }, GridPos { column: 3, row: 0 }];

/// Counter value after a practice starts (0x23B684..0x23B6C8).
pub fn next_practice_counter(counter: u8) -> u8 {
    if counter < 3 {
        let advanced = counter + 1;
        if advanced >= 3 {
            0
        } else {
            advanced
        }
    } else {
        counter
    }
}

pub fn practice_start(counter: u8, decks: &[DeckInput]) -> Result<PracticeStart, PracticeError> {
    let (first, second) = *PRACTICE_PAIRS
        .get(usize::from(counter))
        .ok_or(PracticeError::Counter(counter))?;
    let deck = |index: u8| -> Result<&DeckInput, PracticeError> {
        let deck = decks
            .iter()
            .find(|d| d.index == index)
            .ok_or(PracticeError::MissingDeck(index))?;
        if deck.cards.len() != DECK_SIZE {
            return Err(PracticeError::DeckSize {
                index,
                cards: deck.cards.len(),
            });
        }
        Ok(deck)
    };
    let records = [deck(first)?, deck(second)?];
    let units = records
        .iter()
        .enumerate()
        .map(|(side, deck)| Unit {
            id: UnitId(side as u16),
            card_id: deck.leader_card,
            side: side as u8,
            position: LEADER_CELLS[side],
        })
        .collect();
    let board = CoreState::new(StateDocument {
        schema_version: STATE_SCHEMA.into(),
        terrain: vec![crate::NORMAL_TERRAIN; CELL_COUNT],
        units,
        life_points: [STARTING_LIFE_POINTS; 2],
    })
    .map_err(PracticeError::State)?;
    let sides = records.map(|deck| SideSetup {
        deck_index: deck.index,
        leader_rank_raw: deck.leader_rank_raw,
        draw_pile: deck.cards[OBSERVED_HAND_SIZE..].to_vec(),
        hand: deck.cards[..OBSERVED_HAND_SIZE].to_vec(),
    });
    Ok(PracticeStart {
        counter,
        board,
        sides,
        verified_against_original: counter == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(index: u8, leader: u16) -> DeckInput {
        DeckInput {
            index,
            leader_card: leader,
            leader_rank_raw: 1,
            cards: (100..140).collect(),
        }
    }

    #[test]
    fn counter_selects_the_traced_pairs_and_wraps_at_three() {
        let decks: Vec<_> = (0x11..=0x1A).map(|i| deck(i, u16::from(i))).collect();
        let start = practice_start(0, &decks).unwrap();
        assert_eq!(
            (start.sides[0].deck_index, start.sides[1].deck_index),
            (0x11, 0x12)
        );
        assert_eq!(
            start
                .board
                .unit_at(GridPos { column: 3, row: 6 })
                .unwrap()
                .card_id,
            0x11
        );
        assert_eq!(
            start
                .board
                .unit_at(GridPos { column: 3, row: 0 })
                .unwrap()
                .card_id,
            0x12
        );
        assert_eq!(start.board.document().life_points, [4000, 4000]);
        assert_eq!(start.sides[0].hand, vec![100, 101, 102, 103, 104]);
        assert_eq!(start.sides[0].draw_pile.len(), 35);
        assert!(start.verified_against_original);
        assert!(!practice_start(4, &decks).unwrap().verified_against_original);
        assert_eq!(
            practice_start(5, &decks).unwrap_err(),
            PracticeError::Counter(5)
        );
        assert_eq!(
            practice_start(0, &decks[1..]).unwrap_err(),
            PracticeError::MissingDeck(0x11)
        );
        assert_eq!([0, 1, 2, 3, 4].map(next_practice_counter), [1, 2, 0, 3, 4]);
    }
}
