//! Alternate card art table from the PAL executable.
//!
//! 0x215D10 (art counter cycle, called for "GET SPECIAL CARD" prizes) and
//! 0x215E60 (picture to show) walk a table of 16 six-byte entries at EE
//! 0x2D0FE0: u16 `index | variants << 14` and two u16 picture ids. The
//! first field is not a card id: both functions map it through 0x1DF3A0,
//! which reads the u16 card id at EE 0x2CE024 + 2·index (the library-order
//! to card-id table), and compare the result with the card. Raw data only;
//! the counter rule lives in `roses-core` (`card_block::CardBlock::cycle_art`).
//! [F] read from the code; what the pictures look like is not checked here.

use super::{require_pal_executable, Error, Result, CARD_COUNT, PICTURE_RECORD_COUNT};

/// The alternate-art table in EE memory (16 entries of 6 bytes).
pub const ALTERNATE_ART_EE: u32 = 0x2D_0FE0;
/// Entries of the table (the loop bound of 0x215D10 / 0x215E60).
pub const ALTERNATE_ART_COUNT: usize = 16;
/// The u16[854] table 0x1DF3A0 indexes (library order → card id).
pub const CARD_ORDER_EE: u32 = 0x2C_E024;
const EE_TO_FILE: u32 = 0xF_9680;
const ENTRY_BYTES: usize = 6;

/// One entry, with the first field already resolved to a card id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AlternateArtRecord {
    /// Raw first field & 0x3FFF (the index into the card-order table).
    pub index: u16,
    /// The card the entry applies to (card-order table at `index`).
    pub card: u16,
    /// Raw first field >> 14: how many alternate pictures the entry has
    /// (the art counter cycles over `variants + 1` values).
    pub variants: u8,
    /// Picture record ids of the alternates (999 = unused).
    pub pictures: [u16; 2],
}

/// The 16 entries of the table. Every card must be a real card id and every
/// used picture id must lie in the 871-record picture archive.
pub fn parse_pal_alternate_art(executable: &[u8]) -> Result<Vec<AlternateArtRecord>> {
    require_pal_executable(executable)?;
    let order_start = (CARD_ORDER_EE - EE_TO_FILE) as usize;
    let order = executable
        .get(order_start..order_start + 2 * CARD_COUNT)
        .ok_or_else(|| Error::Invalid("card-order table lies outside the executable".into()))?;
    let start = (ALTERNATE_ART_EE - EE_TO_FILE) as usize;
    let table = executable
        .get(start..start + ENTRY_BYTES * ALTERNATE_ART_COUNT)
        .ok_or_else(|| Error::Invalid("alternate-art table lies outside the executable".into()))?;
    let u16_at = |bytes: &[u8], at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    table
        .chunks_exact(ENTRY_BYTES)
        .map(|entry| {
            let raw = u16_at(entry, 0);
            let index = raw & 0x3FFF;
            let variants = (raw >> 14) as u8;
            let card = *order
                .get(usize::from(index) * 2..)
                .and_then(|o| o.get(..2))
                .map(|o| u16::from_le_bytes([o[0], o[1]]))
                .as_ref()
                .ok_or_else(|| {
                    Error::Invalid("alternate-art index outside the card order".into())
                })?;
            if usize::from(card) >= CARD_COUNT {
                return Err(Error::Invalid("alternate-art entry names no card".into()));
            }
            let pictures = [u16_at(entry, 2), u16_at(entry, 4)];
            for (k, &p) in pictures.iter().enumerate() {
                let used = k < usize::from(variants);
                if used && usize::from(p) >= PICTURE_RECORD_COUNT {
                    return Err(Error::Invalid(
                        "alternate-art picture outside the archive".into(),
                    ));
                }
            }
            Ok(AlternateArtRecord {
                index,
                card,
                variants,
                pictures,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternate_art_needs_the_pal_executable() {
        assert!(matches!(
            parse_pal_alternate_art(&[0; 64]),
            Err(Error::UnsupportedExecutable { .. })
        ));
    }

    #[test]
    #[ignore = "requires the private PAL executable; set ROSES_TEST_ELF_PATH"]
    fn private_alternate_art_parses() {
        let elf =
            std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").expect("ROSES_TEST_ELF_PATH"))
                .unwrap();
        let table = parse_pal_alternate_art(&elf).unwrap();
        assert_eq!(table.len(), ALTERNATE_ART_COUNT);
        // The alternates are the 17 picture records after the 854 cards.
        let alternates: usize = table.iter().map(|e| usize::from(e.variants)).sum();
        assert_eq!(alternates, PICTURE_RECORD_COUNT - CARD_COUNT);
        assert!(table.iter().all(|e| e.variants >= 1));
        let mut pictures: Vec<u16> = table
            .iter()
            .flat_map(|e| e.pictures[..usize::from(e.variants)].to_vec())
            .collect();
        pictures.sort_unstable();
        assert_eq!(
            pictures,
            (CARD_COUNT as u16..PICTURE_RECORD_COUNT as u16).collect::<Vec<_>>()
        );
        // Entry 0 is index 0 = card 0 (the card order starts with card 0).
        assert_eq!((table[0].index, table[0].card), (0, 0));
    }
}
