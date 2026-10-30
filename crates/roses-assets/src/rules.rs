//! Fingerprinted static fusion and terrain tables. These are data records,
//! not implementations of the game's fusion resolution or terrain effects.

use super::{require_pal_executable, Error, Result, CARD_COUNT, PROFILE_ID};
use serde::Serialize;
use std::collections::BTreeSet;

pub const FUSION_COUNT: usize = 26_540;
pub const FUSION_TABLE_FILE_OFFSET: usize = 0x1A45A0;
pub const MAP_COUNT: usize = 46;
pub const MAP_SIDE: usize = 7;
pub const MAP_TABLE_FILE_OFFSET: usize = 0x1D5F60;
const MAP_CELLS: usize = MAP_SIDE * MAP_SIDE;
const FUSION_SENTINEL: u32 = 0x3E7F9FE7;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FusionRecord {
    /// Original table order is preserved; this is not a card ID.
    pub index: u32,
    /// The ingredient IDs are ordered numerically, with equality permitted.
    pub ingredient_low: u16,
    pub ingredient_high: u16,
    pub result_id: u16,
    /// The complete source word, including the currently zero high two bits.
    pub raw_word: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TerrainMap {
    pub index: u16,
    pub width: u8,
    pub height: u8,
    /// Exactly 49 terrain codes in source order. Row-major presentation is a
    /// research interpretation; in-game orientation has not been checked here.
    pub cells: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleTables {
    pub schema_version: u32,
    pub profile_id: String,
    pub executable_sha256: String,
    pub fusion_table_file_offset: u64,
    pub map_table_file_offset: u64,
    /// The observed 999/999/999 marker following the bounded fusion table.
    /// Its runtime role remains unverified; it is not returned as a fusion.
    pub fusion_sentinel_raw: u32,
    pub fusions: Vec<FusionRecord>,
    pub maps: Vec<TerrainMap>,
    pub gameplay_validated: bool,
    pub map_orientation_verified: bool,
}

/// Import the supported PAL executable's 26,540 fusion records and 46 boards.
/// Only the exact known executable is accepted. IDs, packed fields, uniqueness,
/// table bounds and terrain ranges are checked; no game code is executed.
pub fn parse_pal_rules(executable: &[u8]) -> Result<RuleTables> {
    let executable_sha256 = require_pal_executable(executable)?;
    let fusion_end = FUSION_TABLE_FILE_OFFSET + (FUSION_COUNT + 1) * 4;
    let fusion_bytes = executable
        .get(FUSION_TABLE_FILE_OFFSET..fusion_end)
        .ok_or_else(|| Error::Invalid("fusion table lies outside executable".into()))?;
    let map_end = MAP_TABLE_FILE_OFFSET + MAP_COUNT * MAP_CELLS;
    let map_bytes = executable
        .get(MAP_TABLE_FILE_OFFSET..map_end)
        .ok_or_else(|| Error::Invalid("map table lies outside executable".into()))?;
    let (fusions, fusion_sentinel_raw) = parse_fusions(fusion_bytes, FUSION_COUNT)?;
    let maps = parse_maps(map_bytes, MAP_COUNT)?;
    Ok(RuleTables {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        executable_sha256,
        fusion_table_file_offset: FUSION_TABLE_FILE_OFFSET as u64,
        map_table_file_offset: MAP_TABLE_FILE_OFFSET as u64,
        fusion_sentinel_raw,
        fusions,
        maps,
        gameplay_validated: false,
        map_orientation_verified: false,
    })
}

fn parse_fusions(bytes: &[u8], count: usize) -> Result<(Vec<FusionRecord>, u32)> {
    if count == 0 || count > FUSION_COUNT || bytes.len() != (count + 1) * 4 {
        return Err(Error::Invalid(
            "fusion data has an incorrect bounded record count or length".into(),
        ));
    }
    let mut records = Vec::with_capacity(count);
    let mut pairs = BTreeSet::new();
    for (index, record) in bytes[..count * 4].chunks_exact(4).enumerate() {
        let raw_word = u32::from_le_bytes(record.try_into().expect("complete record"));
        if raw_word >> 30 != 0 {
            return Err(Error::Unsupported(format!(
                "fusion {index} uses unknown high bits"
            )));
        }
        let ingredient_low = ((raw_word >> 10) & 0x3FF) as u16;
        let ingredient_high = (raw_word & 0x3FF) as u16;
        let result_id = ((raw_word >> 20) & 0x3FF) as u16;
        if [ingredient_low, ingredient_high, result_id]
            .iter()
            .any(|id| usize::from(*id) >= CARD_COUNT)
        {
            return Err(Error::Invalid(format!(
                "fusion {index} references an out-of-range card"
            )));
        }
        if ingredient_low > ingredient_high {
            return Err(Error::Invalid(format!(
                "fusion {index} has reversed ingredient ordering"
            )));
        }
        if !pairs.insert((ingredient_low, ingredient_high)) {
            return Err(Error::Invalid(format!(
                "fusion {index} duplicates an ingredient pair"
            )));
        }
        records.push(FusionRecord {
            index: index as u32,
            ingredient_low,
            ingredient_high,
            result_id,
            raw_word,
        });
    }
    let sentinel = u32::from_le_bytes(bytes[count * 4..].try_into().expect("complete marker"));
    if sentinel != FUSION_SENTINEL {
        return Err(Error::Invalid(
            "fusion table has an unexpected trailing marker".into(),
        ));
    }
    Ok((records, sentinel))
}

fn parse_maps(bytes: &[u8], count: usize) -> Result<Vec<TerrainMap>> {
    if count == 0 || count > MAP_COUNT || bytes.len() != count * MAP_CELLS {
        return Err(Error::Invalid(
            "map data has an incorrect bounded record count or length".into(),
        ));
    }
    bytes
        .chunks_exact(MAP_CELLS)
        .enumerate()
        .map(|(index, cells)| {
            if cells.iter().any(|cell| *cell > 9) {
                return Err(Error::Invalid(format!(
                    "map {index} contains an unknown terrain code"
                )));
            }
            Ok(TerrainMap {
                index: index as u16,
                width: MAP_SIDE as u8,
                height: MAP_SIDE as u8,
                cells: cells.to_vec(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fusion_bytes(words: &[u32]) -> Vec<u8> {
        words
            .iter()
            .chain(std::iter::once(&FUSION_SENTINEL))
            .flat_map(|word| word.to_le_bytes())
            .collect()
    }

    #[test]
    fn decodes_distinct_bit_fields_and_preserves_source_order() {
        let bytes = fusion_bytes(&[0x35500DFF, 0x200C8355]);
        let (fusions, marker) = parse_fusions(&bytes, 2).unwrap();
        assert_eq!(
            fusions[0],
            FusionRecord {
                index: 0,
                ingredient_low: 3,
                ingredient_high: 511,
                result_id: 853,
                raw_word: 0x35500DFF,
            }
        );
        assert_eq!(
            fusions[1],
            FusionRecord {
                index: 1,
                ingredient_low: 800,
                ingredient_high: 853,
                result_id: 512,
                raw_word: 0x200C8355,
            }
        );
        assert_eq!(marker, FUSION_SENTINEL);
    }

    #[test]
    fn permits_same_ingredient_id_without_accepting_duplicate_pairs() {
        let word = (27 << 20) | (12 << 10) | 12;
        let (fusions, _) = parse_fusions(&fusion_bytes(&[word]), 1).unwrap();
        assert_eq!(fusions[0].ingredient_low, fusions[0].ingredient_high);
        assert!(parse_fusions(&fusion_bytes(&[word, word]), 2).is_err());
        assert!(parse_fusions(&fusion_bytes(&[word, word ^ (1 << 20)]), 2).is_err());
    }

    #[test]
    fn rejects_bad_fusion_bounds_fields_and_sentinel() {
        let valid = fusion_bytes(&[0x35500DFF]);
        assert!(parse_fusions(&valid[..valid.len() - 1], 1).is_err());
        assert!(parse_fusions(&valid, 0).is_err());
        assert!(parse_fusions(&valid, FUSION_COUNT + 1).is_err());
        assert!(parse_fusions(&valid, usize::MAX).is_err());
        assert!(parse_fusions(&valid, 2).is_err());
        for word in [
            0x75500DFF,                  // Unsupported high flag bit.
            (1 << 20) | (25 << 10) | 24, // Reversed pair.
            (854 << 20) | (1 << 10) | 2, // Bad result.
            (1 << 20) | (2 << 10) | 854, // Bad ingredient.
            FUSION_SENTINEL,             // Marker cannot appear inside the records.
        ] {
            assert!(parse_fusions(&fusion_bytes(&[word]), 1).is_err());
        }
        let mut wrong_marker = valid;
        *wrong_marker.last_mut().unwrap() ^= 1;
        assert!(parse_fusions(&wrong_marker, 1).is_err());
    }

    #[test]
    fn map_cells_retain_order_and_separate_board_boundaries() {
        let mut bytes: Vec<_> = (0..49).map(|value| (value % 10) as u8).collect();
        bytes.extend_from_slice(&[9; 49]);
        let maps = parse_maps(&bytes, 2).unwrap();
        assert_eq!((maps[0].index, maps[0].width, maps[0].height), (0, 7, 7));
        assert_eq!(maps[0].cells, bytes[..49]);
        assert_eq!(maps[0].cells[7], 7);
        assert_eq!(maps[1].index, 1);
        assert_eq!(maps[1].cells, [9; 49]);
    }

    #[test]
    fn map_parser_rejects_invalid_cells_and_incomplete_records() {
        assert!(parse_maps(&[0; 48], 1).is_err());
        assert!(parse_maps(&[0; 50], 1).is_err());
        assert!(parse_maps(&[0; 49], 0).is_err());
        assert!(parse_maps(&[0; 49], MAP_COUNT + 1).is_err());
        assert!(parse_maps(&[0; 49], usize::MAX).is_err());
        let mut bad_cells = [0; 49];
        bad_cells[48] = 10;
        assert!(parse_maps(&bad_cells, 1).is_err());
    }

    #[test]
    fn rules_require_the_known_complete_executable() {
        for bytes in [b"".as_slice(), b"\x7FELF".as_slice()] {
            assert!(matches!(
                parse_pal_rules(bytes),
                Err(Error::UnsupportedExecutable { .. })
            ));
        }
    }

    #[test]
    #[ignore = "requires private PAL executable; set ROSES_TEST_ELF_PATH"]
    fn private_pal_rules_decode_with_expected_structural_properties() {
        let path = std::env::var_os("ROSES_TEST_ELF_PATH").expect("set ROSES_TEST_ELF_PATH");
        let executable = std::fs::read(path).unwrap();
        let rules = parse_pal_rules(&executable).unwrap();
        assert_eq!(rules.fusions.len(), 26_540);
        assert_eq!(rules.maps.len(), 46);
        assert_eq!(rules.fusion_sentinel_raw, FUSION_SENTINEL);
        assert_eq!(
            rules
                .fusions
                .iter()
                .filter(|f| f.ingredient_low == f.ingredient_high)
                .count(),
            19
        );
        assert!(rules
            .fusions
            .iter()
            .all(|f| f.ingredient_high <= 669 && (4..=667).contains(&f.result_id)));
        assert!(rules
            .maps
            .iter()
            .all(|m| m.cells.len() == 49 && m.cells.iter().all(|cell| *cell <= 9)));
        assert_eq!(
            rules
                .maps
                .iter()
                .filter(|m| m.cells.iter().all(|cell| *cell == m.cells[0]))
                .count(),
            10
        );
        assert!(!rules.gameplay_validated);
        assert!(!rules.map_orientation_verified);
    }
}
