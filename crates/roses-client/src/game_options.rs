//! The three game options of the card block's flag byte +0xFF9C (Claude,
//! 5 Oct 2026). Notes and traces: `work/research/modes/map-modes/`.
//!
//! [F] 0x214340(side, i, v) writes and 0x214490(side, i) reads bit 2 + i
//! of the block of `side` (0x2F2C10 + 0xFFA0·(side ≠ 0x312B50)); writing
//! i = 2 also calls 0x217200(v), the sound output mode. 0x213DB0 (a new
//! block) sets bits 2..4, so every option starts at its first value.
//!
//! - i = 0, **Card Name: New DM / OFCG notation** (MSG 115/118/119): the
//!   only reader is 0x280E00, the tutorial texts' remap, which reads the
//!   tutorial's own reset buffer (`roses_core::duel::script::remap_text`);
//!   the PAL panels (the map's and the duel's Opt) have no row for it, so
//!   it never changes.
//! - i = 1, **Monster Battle: Display / Abbreviate** (116/120/121): no
//!   code reads it outside the panels, the tutorial's block back-up and
//!   the Custom Duel copy (every `jal 0x214490` and every +0xFF9C load in
//!   the whole 32 MB of the map, duel and attack savestates) — the PAL
//!   battle never consults it, so it changes nothing here either.
//! - i = 2, **Sound: Stereo / Mono** (117/123/122): 0x217200(v) →
//!   0x10A830(v ? 0 : 1), the IOP command 0xF003 (output mode); the boot's
//!   sound init calls 0x217200(1) and the title applies block 0's bit at
//!   0x1AD210. The port downmixes in `sound_mix::Mixer::set_mono`.
use roses_core::card_block::CardBlock;

/// The panel's D36 layout: bit i = block bit 2 + i.
pub const CARD_NAME: u8 = 1 << 0;
pub const BATTLE_DISPLAY: u8 = 1 << 1;
pub const STEREO: u8 = 1 << 2;
/// 0x213DB0: a new block's options (bits 2..4 of +0xFF9C = 0x1C).
pub const DEFAULT_BITS: u8 = CARD_NAME | BATTLE_DISPLAY | STEREO;
const FIRST_BIT: u8 = 2;

/// 0x214490(·, 0..2) as the panel's three bits (D36, 0x1E5270).
pub fn bits(block: &CardBlock) -> u8 {
    block
        .raw_tail
        .get(crate::ending::BLOCK_FLAGS_TAIL_OFFSET)
        .map_or(DEFAULT_BITS, |b| (b >> FIRST_BIT) & DEFAULT_BITS)
}

/// 0x214340(·, i, bit i) for i = 0..2: the other bits of +0xFF9C (the
/// current deck, the both-routes mark) are kept.
pub fn set_bits(block: &mut CardBlock, bits: u8) {
    if let Some(b) = block.raw_tail.get_mut(crate::ending::BLOCK_FLAGS_TAIL_OFFSET) {
        *b = (*b & !(DEFAULT_BITS << FIRST_BIT)) | ((bits & DEFAULT_BITS) << FIRST_BIT);
    }
}

/// 0x217200's argument: the sound row's bit (set = stereo).
pub fn stereo(bits: u8) -> bool {
    bits & STEREO != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_options_are_bits_two_to_four_of_the_flag_byte() {
        let mut block = CardBlock::empty(0);
        block.raw_tail[crate::ending::BLOCK_FLAGS_TAIL_OFFSET] = 0x23; // deck 3, both routes
        assert_eq!(bits(&block), 0);
        set_bits(&mut block, DEFAULT_BITS);
        assert_eq!(block.raw_tail[crate::ending::BLOCK_FLAGS_TAIL_OFFSET], 0x3F);
        // [emu options-toggle] Right on both rows, Cross: 0x1C → 0x04.
        set_bits(&mut block, CARD_NAME);
        assert_eq!(block.raw_tail[crate::ending::BLOCK_FLAGS_TAIL_OFFSET], 0x27);
        assert_eq!(bits(&block), CARD_NAME);
        assert!(!stereo(bits(&block)));
    }
}
