//! The Destiny Draw's card rows from the PAL executable.
//!
//! The low-LP special draw (research: the project notes §3)
//! replaces the last card of a summon refill with one of three ids taken
//! from a fixed row: `u16[3]` at EE 0x2B7AF0 + 6·row, row = the situation
//! 1..=7 in the low nibble of the side's Destiny byte. Row 0 (and the row
//! after 7) holds the 999 filler and is never armed. Raw data only; the pick
//! (leader-type weights, one rand31) lives in `roses-core`
//! (`duel::script::destiny`).

use super::{require_pal_executable, Error, Result, CARD_COUNT};

/// The row table in EE memory (0x270FC0 reads `+ 6·row`).
pub const DESTINY_ROWS_EE: u32 = 0x2B_7AF0;
/// Rows 0..=7 as the pick indexes them (row 0 is filler).
pub const DESTINY_ROW_COUNT: usize = 8;
const EE_TO_FILE: u32 = 0xF_9680;
/// The empty-slot value of the table (and of every card list).
const FILLER: u16 = 999;

/// The rows by situation; `rows[0]` is filler.
pub type DestinyRows = [[u16; 3]; DESTINY_ROW_COUNT];

/// The eight rows, checked so that rows 1..=7 name cards of the table.
pub fn parse_pal_destiny_rows(executable: &[u8]) -> Result<DestinyRows> {
    require_pal_executable(executable)?;
    let start = (DESTINY_ROWS_EE - EE_TO_FILE) as usize;
    let bytes = executable
        .get(start..start + 6 * DESTINY_ROW_COUNT)
        .ok_or_else(|| Error::Invalid("Destiny Draw rows lie outside the executable".into()))?;
    let mut rows = [[0u16; 3]; DESTINY_ROW_COUNT];
    for (row, chunk) in rows.iter_mut().zip(bytes.chunks_exact(6)) {
        for (id, pair) in row.iter_mut().zip(chunk.chunks_exact(2)) {
            *id = u16::from_le_bytes([pair[0], pair[1]]);
        }
    }
    if rows[0] != [FILLER; 3]
        || rows[1..]
            .iter()
            .flatten()
            .any(|&id| usize::from(id) >= CARD_COUNT)
    {
        return Err(Error::Invalid(
            "Destiny Draw rows must start with the filler row and name cards".into(),
        ));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destiny_rows_need_the_pal_executable() {
        assert!(matches!(
            parse_pal_destiny_rows(&[0; 64]),
            Err(Error::UnsupportedExecutable { .. })
        ));
    }

    #[test]
    #[ignore = "requires the private PAL executable; set ROSES_TEST_ELF_PATH"]
    fn private_destiny_rows_parse() {
        let elf =
            std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").expect("ROSES_TEST_ELF_PATH"))
                .unwrap();
        let rows = parse_pal_destiny_rows(&elf).unwrap();
        // SPECIAL-DRAW.md §3: row 2 = Raigeki, Dark Hole, Arsenal Bug (the
        // tutorial's situation); every row of 1..=7 names three cards.
        assert_eq!(rows[2], [700, 699, 429]);
        assert_eq!(rows[0], [999; 3]);
        assert!(rows[1..].iter().flatten().all(|&id| id != 999));
    }
}
