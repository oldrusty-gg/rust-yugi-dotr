//! Card rank thresholds from the PAL executable.
//!
//! 0x215570 (result state 0x5E) recomputes a card's rank as the largest r
//! with experience >= T[r] over the u16[13] table at EE 0x2D0FC0. Raw data
//! only; the rule lives in `roses-core` (`duel::recompute_rank`).

use super::{require_pal_executable, Error, Result};

/// The threshold table in EE memory.
pub const RANK_THRESHOLDS_EE: u32 = 0x2D_0FC0;
/// Ranks 0..=12 (NCO .. SD).
pub const RANK_THRESHOLD_COUNT: usize = 13;
const EE_TO_FILE: u32 = 0xF_9680;

/// The 13 thresholds, checked to be non-decreasing and to start at 0 (so
/// every experience value has a rank).
pub fn parse_pal_rank_thresholds(executable: &[u8]) -> Result<[u16; RANK_THRESHOLD_COUNT]> {
    require_pal_executable(executable)?;
    let start = (RANK_THRESHOLDS_EE - EE_TO_FILE) as usize;
    let bytes = executable
        .get(start..start + 2 * RANK_THRESHOLD_COUNT)
        .ok_or_else(|| Error::Invalid("rank thresholds lie outside the executable".into()))?;
    let mut thresholds = [0u16; RANK_THRESHOLD_COUNT];
    for (t, pair) in thresholds.iter_mut().zip(bytes.chunks_exact(2)) {
        *t = u16::from_le_bytes([pair[0], pair[1]]);
    }
    if thresholds[0] != 0 || thresholds.windows(2).any(|w| w[0] > w[1]) {
        return Err(Error::Invalid(
            "rank thresholds must start at 0 and never decrease".into(),
        ));
    }
    Ok(thresholds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_thresholds_need_the_pal_executable() {
        assert!(matches!(
            parse_pal_rank_thresholds(&[0; 64]),
            Err(Error::UnsupportedExecutable { .. })
        ));
    }

    #[test]
    #[ignore = "requires the private PAL executable; set ROSES_TEST_ELF_PATH"]
    fn private_rank_thresholds_parse() {
        let elf =
            std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").expect("ROSES_TEST_ELF_PATH"))
                .unwrap();
        let t = parse_pal_rank_thresholds(&elf).unwrap();
        // DUEL-END.md §2: 500 for rank 1, the last one unreachable below 0xFFFF.
        assert_eq!((t[0], t[1], t[12]), (0, 500, 0xFFFF));
        assert!(t.windows(2).all(|w| w[0] < w[1]));
    }
}
