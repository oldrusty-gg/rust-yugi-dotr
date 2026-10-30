//! PICMINI.MRG: one 4 KiB slot per card id holding a small RH2 record — the
//! 40x32 mini pictures the duel HUD shows as leader portraits (duel texture
//! map, `work/research/duel/claude-screen/TEXTURES.md`). Unlike the
//! page-based opening archives, each record uploads a 256-entry C_16 palette
//! and one row-major PSMT8 rect. Structure is validated; no pixels are
//! embedded here.

use crate::opening_textures::{color, u32_at, validate_packet};
use crate::{Error, OpeningIndexedTexture, OpeningPaletteFormat, Result, RgbaImage, CARD_COUNT};

pub const MINI_PICTURE_SLOT_BYTES: usize = 4096;
pub const PAL_MINI_PICTURE_ARCHIVE_BYTES: usize = 2_863_104;
const HEADER_BYTES: usize = 0x60;
const PALETTE_START: usize = HEADER_BYTES + 2 * 48;
const PALETTE_BYTES: usize = HEADER_BYTES + 256 * 2;

fn invalid(m: &str) -> Error {
    Error::Invalid(m.into())
}

/// Decode the mini picture of `card` without converting its palette.
pub fn decode_pal_mini_picture(archive: &[u8], card: u16) -> Result<OpeningIndexedTexture> {
    if archive.len() != PAL_MINI_PICTURE_ARCHIVE_BYTES {
        return Err(Error::Unsupported("PICMINI.MRG size".into()));
    }
    let slots = archive.len() / MINI_PICTURE_SLOT_BYTES;
    if usize::from(card) >= slots.min(CARD_COUNT) {
        return Err(invalid("card has no mini picture slot"));
    }
    let start = usize::from(card) * MINI_PICTURE_SLOT_BYTES;
    let bytes = &archive[start..start + MINI_PICTURE_SLOT_BYTES];
    if bytes.get(..4) != Some(b"RH2\0") {
        return Err(invalid("mini picture RH2 header is missing"));
    }
    let word = |i: usize| u32_at(bytes, i * 4);
    let (width, height) = ((word(21)? & 0xFFFF) as usize, (word(21)? >> 16) as usize);
    let pixels = width * height;
    let pixel_payload = pixels.div_ceil(16) * 16;
    let pixel_start = PALETTE_START + PALETTE_BYTES;
    let len = pixel_start + HEADER_BYTES + pixel_payload;
    if word(20)? != 0x0002_0013
        || width == 0
        || height == 0
        || width > 128
        || height > 128
        || word(2)? as usize != len
        || word(4)? as usize != HEADER_BYTES
        || word(5)? != 96
        || word(6)? as usize != PALETTE_START
        || word(7)? as usize != PALETTE_BYTES
        || word(8)? as usize != pixel_start
        || word(9)? as usize != HEADER_BYTES + pixel_payload
        || word(10)? as usize != len
        || word(22)? != 2
        || len > MINI_PICTURE_SLOT_BYTES
    {
        return Err(invalid("mini picture RH2 layout is not the supported form"));
    }
    validate_packet(bytes, PALETTE_START, 512, 16, 16)?;
    validate_packet(
        bytes,
        pixel_start,
        pixel_payload,
        width as u64,
        height as u64,
    )?;
    let palette_words_raw = (0..256)
        .map(|i| {
            let at = PALETTE_START + HEADER_BYTES + i * 2;
            u32::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
        })
        .collect();
    // Linear PSMT8 upload; the CSM1 CLUT swaps index bits 3 and 4.
    let palette_indices = bytes[pixel_start + HEADER_BYTES..][..pixels]
        .iter()
        .map(|&i| (i & !24) | ((i & 8) << 1) | ((i & 16) >> 1))
        .collect();
    Ok(OpeningIndexedTexture {
        width: width as u32,
        height: height as u32,
        palette_format: OpeningPaletteFormat::Rgb5551,
        palette_words_raw,
        palette_indices,
    })
}

/// The mini picture as RGBA (STP bit → alpha 255, as for opening textures).
pub fn decode_pal_mini_picture_rgba(archive: &[u8], card: u16) -> Result<RgbaImage> {
    let t = decode_pal_mini_picture(archive, card)?;
    Ok(RgbaImage {
        width: t.width,
        height: t.height,
        pixels: t
            .palette_indices
            .iter()
            .flat_map(|&i| color(t.palette_words_raw[usize::from(i)], t.palette_format))
            .collect(),
    })
}
