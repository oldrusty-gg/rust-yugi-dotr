//! Rules tables the duel reads from the executable rather than the card
//! catalogue: the leader ability table (EE 0x2C26B8, 20 u16 per card,
//! ABILITY-2.md §1) and the Destiny Draw rows (EE 0x2B7AF0). Each side reads
//! them from its own import's hash-checked executable; only their hash
//! (inside [`crate::data_fingerprint`]) is ever exchanged, so both ends
//! agree on what a leader can do before a match begins.

use roses_core::duel::script::destiny::DestinyRows;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// EE address of the ability table and the ELF offset of EE addresses.
pub const ABILITY_TABLE_EE: usize = 0x2C_26B8;
pub const EE_TO_FILE: usize = 0xF_9680;
/// u16 words per card row.
pub const ABILITY_ROW_WORDS: usize = 20;

pub type AbilityRow = [u16; ABILITY_ROW_WORDS];

/// The executable's rules tables (empty for invented test data).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesTables {
    /// One row per card id (`roses_core::CARD_COUNT` rows), or none.
    pub abilities: Vec<AbilityRow>,
    /// The Destiny Draw rows, when known.
    pub destiny: Option<DestinyRows>,
}

impl RulesTables {
    /// No tables: leaders have no abilities, a Destiny Draw is refused.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Both tables from the user's PAL executable.
    pub fn from_executable(executable: &[u8]) -> Result<Self, String> {
        let start = ABILITY_TABLE_EE - EE_TO_FILE;
        let count = usize::from(roses_core::CARD_COUNT);
        let bytes = executable
            .get(start..start + count * ABILITY_ROW_WORDS * 2)
            .ok_or("the leader ability table lies outside the executable")?;
        let abilities = bytes
            .chunks_exact(ABILITY_ROW_WORDS * 2)
            .map(|row| std::array::from_fn(|i| u16::from_le_bytes([row[2 * i], row[2 * i + 1]])))
            .collect();
        let destiny = roses_assets::parse_pal_destiny_rows(executable).map_err(|e| e.to_string())?;
        Ok(Self {
            abilities,
            destiny: Some(destiny),
        })
    }

    /// The abilities a leader `card` at stored `rank` has (0x1E1EF0's
    /// leader rows; `roses_core::duel::queries::leader_abilities`); none
    /// without a table row.
    pub fn leader_abilities(&self, card: u16, rank: u8) -> Vec<u8> {
        self.abilities
            .get(usize::from(card))
            .map_or_else(Vec::new, |row| roses_core::duel::queries::leader_abilities(row, i32::from(rank)))
    }

    /// Feed the tables into a fingerprint.
    pub(crate) fn digest(&self, hasher: &mut Sha256) {
        hasher.update(b"abilities");
        hasher.update((self.abilities.len() as u32).to_le_bytes());
        for row in &self.abilities {
            for w in row {
                hasher.update(w.to_le_bytes());
            }
        }
        match &self.destiny {
            Some(rows) => {
                hasher.update(b"destiny");
                for w in rows.iter().flatten() {
                    hasher.update(w.to_le_bytes());
                }
            }
            None => hasher.update(b"no-destiny"),
        }
    }
}
