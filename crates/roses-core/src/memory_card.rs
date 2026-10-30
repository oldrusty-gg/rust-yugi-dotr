//! The memory-card save file of PAL SLES_515.79 (`BESLES-51579/BESLES-51579`),
//! as the card layer builds and checks it (Claude, 4 Oct 2026; notes in
//! `work/research/story/claude-save/NOTES.md` §3).
//!
//! Layout (0x100F0 bytes, one file per memory card) [F 0x23DD4C (write),
//! 0x23F600 (read); emu: a file written by the game's own code in the EE
//! interpreter with a virtual card matched this layout and checksum]:
//!
//! | offset | size | content |
//! |---|---|---|
//! | 0x0000 | u16 | format word 0x312F70 (0x0001: the card init 0x23BCE0 stores byte 1) |
//! | 0x0002 | u16 | checksum (0x240D70) |
//! | 0x0004 | 0xFFA0 | the player's card block 0x213D70(0) |
//! | 0xFFA4 | 0x14C | the campaign record SAVE (EE 0x2991F0..0x29933C) |
//!
//! Checksum 0x240D70: word 1 is zeroed, the 0x8078 little-endian u16 words
//! of the file are XORed, the result XOR 0x4C6B is stored in word 1; the
//! check recomputes it and compares with the stored word [F]. The reader
//! copies the block and SAVE out of the buffer *before* it tests the
//! checksum [F 0x23F608..0x23F6F8], so [`SaveFile::parse`] returns the
//! contents together with the verdict.
//!
//! Nothing here is a game byte: the constants are sizes, offsets and the
//! checksum key read from the code.

/// Size of the save file (and of the card layer's buffer per port).
pub const FILE_BYTES: usize = 0x100F0;
/// Size of the campaign record SAVE (0x2991F0..0x29933C).
pub const SAVE_BYTES: usize = 0x14C;
/// Size of the player's card block.
pub const BLOCK_BYTES: usize = 0xFFA0;
/// Offset of the card block in the file.
pub const BLOCK_OFFSET: usize = 4;
/// Offset of SAVE in the file.
pub const SAVE_OFFSET: usize = BLOCK_OFFSET + BLOCK_BYTES;
/// Word 0 as the game writes it (0x312F70: byte 1, high byte 0).
pub const FORMAT_WORD: u16 = 0x0001;
/// 0x240D70's final XOR.
pub const CHECKSUM_KEY: u16 = 0x4C6B;
/// The PAL directory and data-file name (region table 0x29A520, entry 3).
pub const PAL_NAME: &str = "BESLES-51579";
/// Free space (KB) a card needs before a first save creates the
/// directory [emu: 249 refused with message 3528, 250 accepted].
pub const MIN_FREE_KB: u32 = 250;

/// 0x240D70 over a whole file image (word 1 counts as zero).
pub fn checksum(file: &[u8]) -> u16 {
    file.chunks_exact(2)
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .fold(0u16, |x, (_, w)| x ^ u16::from_le_bytes([w[0], w[1]]))
        ^ CHECKSUM_KEY
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveFileError {
    Length(usize),
    BlockLength(usize),
}

impl std::fmt::Display for SaveFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Length(n) => write!(f, "a save file is {FILE_BYTES:#x} bytes, not {n:#x}"),
            Self::BlockLength(n) => write!(f, "a card block is {BLOCK_BYTES:#x} bytes, not {n:#x}"),
        }
    }
}

/// The decoded file: what the reader copies back into memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveFile {
    /// Word 0 (back into 0x312F70).
    pub format: u16,
    /// The player's card block image (`CardBlock::to_bytes`).
    pub block: Vec<u8>,
    /// SAVE (0x14C bytes).
    pub save: [u8; SAVE_BYTES],
}

/// [`SaveFile::parse`]'s result: the contents and 0x240D70's verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedSave {
    pub file: SaveFile,
    pub checksum_ok: bool,
}

impl SaveFile {
    pub fn new(block: Vec<u8>, save: [u8; SAVE_BYTES]) -> Result<Self, SaveFileError> {
        if block.len() != BLOCK_BYTES {
            return Err(SaveFileError::BlockLength(block.len()));
        }
        Ok(Self {
            format: FORMAT_WORD,
            block,
            save,
        })
    }

    /// The file as 0x23DD4C builds it, checksum included.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SaveFileError> {
        if self.block.len() != BLOCK_BYTES {
            return Err(SaveFileError::BlockLength(self.block.len()));
        }
        let mut out = vec![0u8; FILE_BYTES];
        out[0..2].copy_from_slice(&self.format.to_le_bytes());
        out[BLOCK_OFFSET..SAVE_OFFSET].copy_from_slice(&self.block);
        out[SAVE_OFFSET..].copy_from_slice(&self.save);
        let sum = checksum(&out);
        out[2..4].copy_from_slice(&sum.to_le_bytes());
        Ok(out)
    }

    /// Split a file image; the checksum verdict is returned beside the
    /// contents (the game has already copied them when it tests it).
    pub fn parse(bytes: &[u8]) -> Result<ParsedSave, SaveFileError> {
        if bytes.len() != FILE_BYTES {
            return Err(SaveFileError::Length(bytes.len()));
        }
        let stored = u16::from_le_bytes([bytes[2], bytes[3]]);
        let mut save = [0u8; SAVE_BYTES];
        save.copy_from_slice(&bytes[SAVE_OFFSET..]);
        Ok(ParsedSave {
            file: Self {
                format: u16::from_le_bytes([bytes[0], bytes[1]]),
                block: bytes[BLOCK_OFFSET..SAVE_OFFSET].to_vec(),
                save,
            },
            checksum_ok: checksum(bytes) == stored,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SaveFile {
        let block: Vec<u8> = (0..BLOCK_BYTES).map(|i| (i * 7 % 251) as u8).collect();
        let mut save = [0u8; SAVE_BYTES];
        save[0x3E] = 3; // wins
        save[0xC2] = 0x2F; // event 303
        save[0xC3] = 0x01;
        SaveFile::new(block, save).unwrap()
    }

    #[test]
    fn the_layout_puts_block_and_save_where_the_card_layer_copies_them() {
        let bytes = sample().to_bytes().unwrap();
        assert_eq!(bytes.len(), 0x100F0);
        assert_eq!(&bytes[0..2], &[1, 0]);
        assert_eq!(bytes[4], 0);
        assert_eq!(bytes[5], 7);
        assert_eq!(bytes[0xFFA4 + 0x3E], 3);
        assert_eq!(u16::from_le_bytes([bytes[0xFFA4 + 0xC2], bytes[0xFFA4 + 0xC3]]), 303);
    }

    #[test]
    fn the_checksum_is_the_xor_of_the_words_with_word_1_zeroed_and_the_key() {
        let bytes = sample().to_bytes().unwrap();
        let mut x = 0u16;
        for (i, w) in bytes.chunks_exact(2).enumerate() {
            if i != 1 {
                x ^= u16::from_le_bytes([w[0], w[1]]);
            }
        }
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), x ^ 0x4C6B);
        // A tiny hand-checked case: a file of zeros but word 0 = 1.
        let mut zero = vec![0u8; FILE_BYTES];
        zero[0] = 1;
        assert_eq!(checksum(&zero), 1 ^ 0x4C6B);
    }

    #[test]
    fn a_file_round_trips_and_a_flipped_byte_fails_the_check_but_still_parses() {
        let file = sample();
        let mut bytes = file.to_bytes().unwrap();
        let parsed = SaveFile::parse(&bytes).unwrap();
        assert!(parsed.checksum_ok);
        assert_eq!(parsed.file, file);
        bytes[0x10] ^= 0xFF;
        let bad = SaveFile::parse(&bytes).unwrap();
        assert!(!bad.checksum_ok);
        assert_eq!(bad.file.save, file.save);
        assert_eq!(SaveFile::parse(&bytes[1..]), Err(SaveFileError::Length(FILE_BYTES - 1)));
    }

    /// A file written by the original's own save code (EE interpreter with
    /// the virtual card: `work/research/story/claude-save/tools/run_save.py`).
    /// Set ROSES_SAVE_ORACLE to that file; skipped otherwise (private data).
    #[test]
    fn private_a_file_written_by_the_game_checks_and_round_trips() {
        let Some(path) = std::env::var_os("ROSES_SAVE_ORACLE") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        let parsed = SaveFile::parse(&bytes).unwrap();
        assert!(parsed.checksum_ok);
        assert_eq!(parsed.file.format, FORMAT_WORD);
        assert_eq!(parsed.file.to_bytes().unwrap(), bytes);
    }
}
