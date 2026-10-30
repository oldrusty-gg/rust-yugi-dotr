//! Fingerprinted static card-effect entries from the PAL executable.
//!
//! Unit creation 0x1DDFE0 copies, for card ids below 0x2AB, five 8-byte
//! entries from `0x2C9170 + index * 40` into unit +0x08..+0x2F, where index
//! is the card record halfword +4; other cards copy one entry from
//! `0x2CB970 + index * 8`. A first word of 0xFFFFFFFF marks an empty entry.
//! Entry roles used by duel code (research, not decoded semantics): 0 battle
//! pre-pass, 1 movement, 2 continuous (face-up defence), 3 flip, 4 destroy.
//! These are raw data records; no effect behaviour is implemented here.

use super::{
    require_pal_executable, Error, Result, CARD_COUNT, CARD_RECORD_STRIDE, CARD_TABLE_FILE_OFFSET,
    PROFILE_ID,
};
use serde::Serialize;

pub const FIRST_NON_MONSTER_ID: u16 = 0x2AB;
pub const MONSTER_EFFECT_FILE_OFFSET: usize = 0x1C_FAF0; // EE 0x2C9170
pub const NON_MONSTER_EFFECT_FILE_OFFSET: usize = 0x1D_22F0; // EE 0x2CB970
const MONSTER_ENTRIES: usize = 5;
const ENTRY_BYTES: usize = 8;
const MAX_MONSTER_INDEX: usize = 256;
const MAX_NON_MONSTER_INDEX: usize = 256;
const NO_EFFECT: u16 = 0xFFFF;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct EffectEntry {
    pub word0: u32,
    pub word1: u32,
}
impl EffectEntry {
    /// The type byte used by the traced dispatchers (word0 >> 8).
    pub fn type_code(&self) -> u8 {
        (self.word0 >> 8) as u8
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CardEffects {
    pub id: u16,
    pub entries: [Option<EffectEntry>; MONSTER_ENTRIES],
}

#[derive(Clone, Debug, Serialize)]
pub struct EffectTable {
    pub schema_version: u32,
    pub profile_id: String,
    pub executable_sha256: String,
    pub cards: Vec<CardEffects>,
    pub gameplay_validated: bool,
}

pub fn parse_pal_card_effects(executable: &[u8]) -> Result<EffectTable> {
    let executable_sha256 = require_pal_executable(executable)?;
    let cards = (0..CARD_COUNT as u16)
        .map(|id| card_effects(executable, id))
        .collect::<Result<Vec<_>>>()?;
    Ok(EffectTable {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        executable_sha256,
        cards,
        gameplay_validated: false,
    })
}

fn entry(bytes: &[u8], at: usize) -> Result<Option<EffectEntry>> {
    let raw = bytes
        .get(at..at + ENTRY_BYTES)
        .ok_or_else(|| Error::Invalid("effect entry lies outside executable".into()))?;
    let word0 = u32::from_le_bytes(raw[0..4].try_into().unwrap());
    let word1 = u32::from_le_bytes(raw[4..8].try_into().unwrap());
    Ok((word0 != u32::MAX).then_some(EffectEntry { word0, word1 }))
}

fn card_effects(executable: &[u8], id: u16) -> Result<CardEffects> {
    let record = CARD_TABLE_FILE_OFFSET + usize::from(id) * CARD_RECORD_STRIDE;
    let index = u16::from_le_bytes([executable[record + 4], executable[record + 5]]);
    let mut entries = [None; MONSTER_ENTRIES];
    if id < FIRST_NON_MONSTER_ID {
        if index != NO_EFFECT {
            if usize::from(index) >= MAX_MONSTER_INDEX {
                return Err(Error::Invalid(format!(
                    "card {id} effect index out of range"
                )));
            }
            let base =
                MONSTER_EFFECT_FILE_OFFSET + usize::from(index) * MONSTER_ENTRIES * ENTRY_BYTES;
            for (slot, value) in entries.iter_mut().enumerate() {
                *value = entry(executable, base + slot * ENTRY_BYTES)?;
            }
        }
    } else if index != NO_EFFECT {
        if usize::from(index) >= MAX_NON_MONSTER_INDEX {
            return Err(Error::Invalid(format!(
                "card {id} effect index out of range"
            )));
        }
        entries[0] = entry(
            executable,
            NON_MONSTER_EFFECT_FILE_OFFSET + usize::from(index) * ENTRY_BYTES,
        )?;
    }
    Ok(CardEffects { id, entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_entries_use_the_all_ones_sentinel() {
        let mut bytes = vec![0xFF; 16];
        bytes[8..12].copy_from_slice(&0x39FD_u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(entry(&bytes, 0).unwrap(), None);
        let e = entry(&bytes, 8).unwrap().unwrap();
        assert_eq!((e.word0, e.word1, e.type_code()), (0x39FD, 0, 0x39));
        assert!(entry(&bytes, 12).is_err());
    }
}
