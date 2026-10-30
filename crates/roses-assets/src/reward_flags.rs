//! Reward-reel card flags from the PAL executable's card table.
//!
//! The reward slot machine (mode 0xF, private research
//! `claude-end/DUEL-END.md` §3) reads two bits of byte 0xB of each 20-byte
//! card record at EE 0x2BE400 (`CARD_TABLE_EE_ADDRESS`, record pointer
//! 0x1DE350):
//! - bit 5: the card may appear on a reel (0x252980 removes every graveyard
//!   card without it; 204 cards lack it) [F];
//! - bit 6: the card is in the random pool of the special-prize mapping
//!   0x252E10 (29 cards; 7 of them are also fixed mapping results and are
//!   excluded from the pool by that function, whose eighth result 733 has
//!   no bit 6) [F].
//!
//! Raw data only; the rules live in `roses-core::slot_machine`.

use super::{
    require_pal_executable, Error, Result, CARD_COUNT, CARD_RECORD_STRIDE, CARD_TABLE_FILE_OFFSET,
};

/// Offset of the flag byte inside a card record.
pub const REWARD_FLAG_BYTE: usize = 0x0B;
/// Bit of byte 0xB that allows a card on a reel.
pub const REEL_ELIGIBLE_BIT: u8 = 1 << 5;
/// Bit of byte 0xB that puts a card in the special-prize random pool.
pub const SPECIAL_POOL_BIT: u8 = 1 << 6;

/// One entry per card id (854), in table order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewardFlagTable {
    /// Byte 0xB of each record, unchanged.
    pub flag_bytes: Vec<u8>,
}

impl RewardFlagTable {
    /// Bit 5 of byte 0xB.
    pub fn reel_eligible(&self, card: u16) -> bool {
        self.flag_bytes
            .get(usize::from(card))
            .is_some_and(|b| b & REEL_ELIGIBLE_BIT != 0)
    }
    /// Bit 6 of byte 0xB.
    pub fn special_pool(&self, card: u16) -> bool {
        self.flag_bytes
            .get(usize::from(card))
            .is_some_and(|b| b & SPECIAL_POOL_BIT != 0)
    }
}

/// Byte 0xB of all 854 card records, only from the authenticated executable.
pub fn parse_pal_reward_flags(executable: &[u8]) -> Result<RewardFlagTable> {
    require_pal_executable(executable)?;
    let table_end = CARD_TABLE_FILE_OFFSET + CARD_COUNT * CARD_RECORD_STRIDE;
    let table = executable
        .get(CARD_TABLE_FILE_OFFSET..table_end)
        .ok_or_else(|| Error::Invalid("card table lies outside executable".into()))?;
    let flag_bytes = table
        .chunks_exact(CARD_RECORD_STRIDE)
        .map(|record| record[REWARD_FLAG_BYTE])
        .collect();
    Ok(RewardFlagTable { flag_bytes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_flags_need_the_pal_executable() {
        assert!(matches!(
            parse_pal_reward_flags(&[0; 64]),
            Err(Error::UnsupportedExecutable { .. })
        ));
    }

    #[test]
    fn flag_bits_are_read_from_byte_0xb() {
        let table = RewardFlagTable {
            flag_bytes: vec![0x20, 0x40, 0x60, 0x05],
        };
        assert!(table.reel_eligible(0) && !table.special_pool(0));
        assert!(!table.reel_eligible(1) && table.special_pool(1));
        assert!(table.reel_eligible(2) && table.special_pool(2));
        assert!(!table.reel_eligible(3) && !table.special_pool(3));
        assert!(!table.reel_eligible(4) && !table.special_pool(4));
    }

    #[test]
    #[ignore = "requires the private PAL executable; set ROSES_TEST_ELF_PATH"]
    fn private_reward_flags_parse() {
        let elf =
            std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").expect("ROSES_TEST_ELF_PATH"))
                .unwrap();
        let t = parse_pal_reward_flags(&elf).unwrap();
        assert_eq!(t.flag_bytes.len(), CARD_COUNT);
        // DUEL-END.md §3: 204 cards lack bit 5; in Weevil's deck only 409
        // (Dungeon Worm). The emu 767 reel cards and 820 (Fake Trap) have it.
        let ineligible = (0..CARD_COUNT as u16)
            .filter(|&c| !t.reel_eligible(c))
            .count();
        assert_eq!(ineligible, 204);
        assert!(!t.reel_eligible(409));
        for c in [397, 817, 406, 757, 809, 820] {
            assert!(t.reel_eligible(c), "card {c}");
        }
        // 22 pool cards + 7 of the 8 fixed mapping results carry bit 6.
        let pool = (0..CARD_COUNT as u16)
            .filter(|&c| t.special_pool(c))
            .count();
        assert_eq!(pool, 29);
        for c in [811, 700, 702, 715, 699, 83, 87] {
            assert!(t.special_pool(c), "mapping result {c}");
        }
        assert!(!t.special_pool(733));
    }
}
