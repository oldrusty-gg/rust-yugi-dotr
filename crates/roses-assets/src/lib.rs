//! Local conversion of a narrowly supported PAL card table and picture format.
//!
//! No original card records or pictures are embedded. Catalogue parsing requires
//! an exact executable fingerprint. Picture decoding validates the fixed format,
//! but does not authenticate an archive's provenance or its association with a
//! particular executable. Callers must obtain both from the inspected local ISO.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;

mod alternate_art;
mod board_geometry;
mod campaign_map_geometry;
mod decks;
mod destiny;
mod effects;
mod equip;
pub mod library;
mod mini_pictures;
mod monster_models;
mod movie;
pub mod sound;
pub mod opening_scene;
mod opening_textures;
mod ranks;
mod reward_flags;
mod rituals;
mod rules;
mod story_geometry;
mod story_music;
mod text;
pub use alternate_art::{
    parse_pal_alternate_art, AlternateArtRecord, ALTERNATE_ART_COUNT, ALTERNATE_ART_EE,
    CARD_ORDER_EE,
};
pub use board_geometry::{
    decode_pal_duel_board, decode_pal_duel_board_texture, BoardDraw, BoardModel, BoardRole,
    BoardStrip, DuelBoardGeometry, UnsupportedBoardRecord, PAL_DUEL_ARENA_OFFSETS,
    PAL_DUEL_ARENA_UNSUPPORTED_OFFSETS, PAL_DUEL_TERRAIN_BLOCK_BYTES,
    PAL_DUEL_TERRAIN_BLOCK_OFFSET, PAL_DUEL_TERRAIN_COUNT, PAL_DUEL_TERRAIN_FIRST_ORDINAL,
    PAL_DUEL_WINDOW_COUNT, PAL_DUEL_WINDOW_FIRST_OFFSET, PAL_DUEL_WINDOW_FIRST_ORDINAL,
    PAL_DUEL_WINDOW_KINDS, PAL_DUEL_WINDOW_STRIDE,
};
pub use campaign_map_geometry::{
    decode_pal_campaign_map, decode_pal_campaign_map_texture, decode_pal_model_bank, CampaignMapGeometry,
    CampaignMapModel, MapDraw, MapStrip, UnsupportedCampaignMapRecord, PAL_CAMPAIGN_MAP_BLOCK,
    PAL_CAMPAIGN_MAP_BLOCK_END, PAL_CAMPAIGN_MAP_BLOCK_OFFSET, PAL_CAMPAIGN_MAP_FIRST_ORDINAL,
    PAL_CAMPAIGN_MAP_FIRST_SLOT, PAL_CAMPAIGN_MAP_MODEL_COUNT,
};
pub use decks::{
    parse_pal_decks, DeckRecord, DeckTable, DECK_CARDS, DECK_COUNT, DECK_TABLE_EE_ADDRESS,
    DECK_TABLE_FILE_OFFSET,
};
pub use destiny::{parse_pal_destiny_rows, DestinyRows, DESTINY_ROWS_EE, DESTINY_ROW_COUNT};
pub use effects::{
    parse_pal_card_effects, CardEffects, EffectEntry, EffectTable, FIRST_NON_MONSTER_ID,
};
pub use equip::{parse_pal_equips, EquipTable, PowerUpData, FIRST_POWER_UP_ID, LAST_POWER_UP_ID};
pub use mini_pictures::{
    decode_pal_mini_picture, decode_pal_mini_picture_rgba, MINI_PICTURE_SLOT_BYTES,
    PAL_MINI_PICTURE_ARCHIVE_BYTES,
};
pub use monster_models::{
    decode_pal_monster_slot, decode_pal_monster_texture, monster_gs_texels, parse_pal_sine_table,
    MonsterAngleKey, MonsterDraw, MonsterEffectTexture, MonsterInfluence, MonsterMatrixSlot,
    MonsterModel, MonsterMotion, MonsterNode, MonsterPass, MonsterRawRecord, MonsterSection,
    MonsterSoundCue, MonsterStrip, MonsterTexture, MonsterTextureRecord, MonsterTextureRef,
    MonsterTrack, MonsterVecKey, MonsterVertex, MONSTER_MOTION_ATTACK, MONSTER_MOTION_DAMAGE,
    MONSTER_MOTION_DESTROYED, MONSTER_MOTION_IDLE, PAL_MONSTER_ARCHIVE_BYTES,
    PAL_MONSTER_ARCHIVE_ISO_PATH, PAL_MONSTER_ARCHIVE_SHA256, PAL_MONSTER_SLOT_BYTES,
    PAL_MONSTER_SLOT_COUNT, PAL_SINE_TABLE_EE_ADDRESS, PAL_SINE_TABLE_ENTRIES,
    PAL_SINE_TABLE_FILE_OFFSET,
};
pub use movie::{
    demux_pal_opening_movie, OpeningMovie, OpeningMovieMetadata, PAL_OPENING_MOVIE_SHA256,
};
pub use opening_textures::{
    decode_pal_opening_indexed_texture, decode_pal_opening_texture, inspect_pal_opening_archive,
    OpeningArchiveKind, OpeningArchivePadding, OpeningIndexedTexture, OpeningPaletteFormat,
    OpeningTextureArchive, OpeningTextureRecord, MAX_OPENING_ARCHIVE_BYTES,
};
pub use ranks::{parse_pal_rank_thresholds, RANK_THRESHOLDS_EE, RANK_THRESHOLD_COUNT};
pub use reward_flags::{
    parse_pal_reward_flags, RewardFlagTable, REEL_ELIGIBLE_BIT, REWARD_FLAG_BYTE, SPECIAL_POOL_BIT,
};
pub use rituals::{parse_pal_rituals, RitualData, RitualTable, RITUAL_REQUIREMENTS_EE};
pub use rules::{
    parse_pal_rules, FusionRecord, RuleTables, TerrainMap, FUSION_COUNT, FUSION_TABLE_FILE_OFFSET,
    MAP_COUNT, MAP_SIDE, MAP_TABLE_FILE_OFFSET,
};
pub use story_geometry::{
    decode_pal_story_geometry, decode_pal_story_model_texture, StoryDraw, StoryGeometry,
    StoryModel, StoryStrip, StoryVertex, PAL_MODEL_ARCHIVE_SHA256,
};
pub use story_music::{
    decode_pal_simon_music, decode_pal_story_music, StoryMusic, StoryMusicMetadata,
    PAL_PCMLIST_SHA256, PAL_SIMON_MUSIC_ISO_PATH, PAL_SIMON_MUSIC_SHA256, PAL_STORY_MUSIC_ISO_PATH,
    PAL_STORY_MUSIC_SHA256,
};
pub use text::{
    decode_pal_message_font, decode_pal_name_font, parse_pal_card_names,
    parse_pal_message_templates, parse_pal_messages, resolve_pal_message_template, CardName,
    CardNames, MessageFont, MessageFontMetadata, MessageLine, MessageSource, MessageTemplateToken,
    MessageToken, PalMessage, PalMessageTemplate, PalMessageTemplates, PalMessages,
    CARD_NAME_FIRST_MESSAGE, MESSAGE_COUNT, PAL_ENGLISH_MESSAGE_SHA256,
};

pub const CARD_COUNT: usize = 854;
pub const CARD_RECORD_STRIDE: usize = 20;
pub const CARD_TABLE_FILE_OFFSET: usize = 0x1C4D80;
pub const CARD_TABLE_EE_ADDRESS: u32 = 0x002B_E400;
pub const PICTURE_RECORD_COUNT: usize = 871;
pub const PICTURE_RECORD_STRIDE: usize = 0x4800;
pub const PAL_EXECUTABLE_SHA256: &str =
    "0b23138866f478cd6ef69190388925b32b1e105635b09298b3a1ddd91549c1a2";
const PROFILE_ID: &str = "dotr-europe-sles-51579";
const PICTURE_CONTENT_BYTES: usize = 0x4410;
const PICTURE_SIDE: usize = 128;
const PICTURE_PIXELS: usize = PICTURE_SIDE * PICTURE_SIDE;
const PALETTE_OFFSET: usize = 0x150;
const FIRST_PIXELS: usize = 0x3B0;
const SECOND_PIXELS: usize = 0x2410;
const PIXEL_HALF_BYTES: usize = PICTURE_PIXELS / 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Invalid(String),
    Unsupported(String),
    UnsupportedExecutable { actual_sha256: String },
    UnsupportedMessageFile { actual_sha256: String },
    PictureIndexOutOfRange { index: usize, count: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "invalid asset data: {message}"),
            Self::Unsupported(message) => write!(f, "unsupported asset format: {message}"),
            Self::UnsupportedExecutable { actual_sha256 } => {
                write!(f, "unsupported PAL executable SHA-256: {actual_sha256}")
            }
            Self::UnsupportedMessageFile { actual_sha256 } => {
                write!(
                    f,
                    "unsupported PAL English message SHA-256: {actual_sha256}"
                )
            }
            Self::PictureIndexOutOfRange { index, count } => {
                write!(
                    f,
                    "picture index {index} is outside archive record count {count}"
                )
            }
        }
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CardRecord {
    /// Zero-based position in the constant table, not a table field.
    pub id: u16,
    /// Raw kind code. Numeric meanings follow the documented game enum.
    pub kind: u8,
    pub kind_offset_raw: u8,
    pub level: u8,
    /// Raw attribute code; non-monster records do not imply a monster attribute.
    pub attribute: u8,
    pub deck_cost: u8,
    /// Preserve sentinel values and avoid treating this as implemented behavior.
    pub effect_id_raw: u16,
    /// Public documentation calls this xaxId; its meaning is not established here.
    pub xax_id_raw: u16,
    pub attack: u16,
    pub defense: u16,
    /// Includes the upper three flag bits, whose behavior is not validated here.
    pub attack_word_raw: u16,
    pub defense_word_raw: u16,
    /// Bytes 12..20 are retained without interpreting their encoding.
    pub opaque_tail: [u8; 8],
}

#[derive(Clone, Debug, Serialize)]
pub struct CardCatalog {
    pub schema_version: u32,
    pub profile_id: String,
    pub executable_sha256: String,
    pub table_file_offset: u64,
    pub record_stride: u32,
    pub cards: Vec<CardRecord>,
    /// Always false: static field decoding is not a duel-behavior test.
    pub gameplay_validated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// Top-to-bottom rows, four bytes per pixel, in RGBA order.
    pub pixels: Vec<u8>,
}

/// Parse all 854 constant records only when the complete executable is known.
/// The decoder never executes input bytes or follows input pointers.
pub fn parse_pal_catalog(executable: &[u8]) -> Result<CardCatalog> {
    let executable_sha256 = require_pal_executable(executable)?;
    let table_end = CARD_TABLE_FILE_OFFSET + CARD_COUNT * CARD_RECORD_STRIDE;
    let table = executable
        .get(CARD_TABLE_FILE_OFFSET..table_end)
        .ok_or_else(|| Error::Invalid("card table lies outside executable".into()))?;
    let cards = parse_card_table(table)?;
    Ok(CardCatalog {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        executable_sha256,
        table_file_offset: CARD_TABLE_FILE_OFFSET as u64,
        record_stride: CARD_RECORD_STRIDE as u32,
        cards,
        gameplay_validated: false,
    })
}

fn require_pal_executable(executable: &[u8]) -> Result<String> {
    let executable_sha256 = format!("{:x}", Sha256::digest(executable));
    if executable_sha256 != PAL_EXECUTABLE_SHA256 {
        return Err(Error::UnsupportedExecutable {
            actual_sha256: executable_sha256,
        });
    }
    Ok(executable_sha256)
}

fn parse_card_table(table: &[u8]) -> Result<Vec<CardRecord>> {
    if table.len() != CARD_COUNT * CARD_RECORD_STRIDE {
        return Err(Error::Invalid(
            "card table must contain exactly 854 complete records".into(),
        ));
    }
    table
        .chunks_exact(CARD_RECORD_STRIDE)
        .enumerate()
        .map(|(id, record)| parse_card_record(id as u16, record))
        .collect()
}

fn parse_card_record(id: u16, record: &[u8]) -> Result<CardRecord> {
    if record.len() != CARD_RECORD_STRIDE {
        return Err(Error::Invalid(
            "card record must be exactly 20 bytes".into(),
        ));
    }
    let kind = record[0];
    let level = record[2] >> 4;
    let attribute = record[2] & 15;
    if !matches!(kind, 0..=20 | 32 | 64 | 96 | 128 | 160) {
        return Err(Error::Invalid(format!(
            "card {id} has an unknown kind code"
        )));
    }
    if level > 12 || attribute > 5 {
        return Err(Error::Invalid(format!(
            "card {id} has an invalid level/attribute"
        )));
    }
    let attack_word_raw = le_u16(record, 8);
    let defense_word_raw = le_u16(record, 10);
    let mut opaque_tail = [0; 8];
    opaque_tail.copy_from_slice(&record[12..20]);
    Ok(CardRecord {
        id,
        kind,
        kind_offset_raw: record[1],
        level,
        attribute,
        deck_cost: record[3],
        effect_id_raw: le_u16(record, 4),
        xax_id_raw: le_u16(record, 6),
        attack: attack_word_raw & 0x1FFF,
        defense: defense_word_raw & 0x1FFF,
        attack_word_raw,
        defense_word_raw,
        opaque_tail,
    })
}

/// Decode one 128x128 indexed picture from an aligned RH2 record archive.
/// The supported PAL archive contains 871 records. Smaller aligned archives are
/// accepted for isolated research and synthetic tests; incomplete records and
/// archives exceeding the known count are rejected. Only the requested record
/// is decoded. A valid picture index is not automatically a verified card ID.
pub fn decode_picture(archive: &[u8], index: usize) -> Result<RgbaImage> {
    if archive.is_empty() || !archive.len().is_multiple_of(PICTURE_RECORD_STRIDE) {
        return Err(Error::Invalid(
            "picture archive contains an incomplete record".into(),
        ));
    }
    let count = archive.len() / PICTURE_RECORD_STRIDE;
    if count > PICTURE_RECORD_COUNT {
        return Err(Error::Unsupported(
            "picture archive exceeds the supported PAL record count".into(),
        ));
    }
    if index >= count {
        return Err(Error::PictureIndexOutOfRange { index, count });
    }
    let start = index * PICTURE_RECORD_STRIDE;
    decode_picture_record(&archive[start..start + PICTURE_RECORD_STRIDE])
}

/// Decode a picture as the GS samples it: RGB5 expanded by a left shift of
/// three and the alpha bit as 128 (TEXA TA1), not 255.
pub fn decode_picture_gs(archive: &[u8], index: usize) -> Result<RgbaImage> {
    let mut image = decode_picture(archive, index)?;
    for p in image.pixels.chunks_exact_mut(4) {
        for c in &mut p[..3] {
            // `rgba5551` expanded v as v·255/31; recover v.
            *c = (((u16::from(*c) * 31 + 127) / 255) << 3) as u8;
        }
        p[3] = if p[3] != 0 { 128 } else { 0 };
    }
    Ok(image)
}

fn decode_picture_record(record: &[u8]) -> Result<RgbaImage> {
    if record.len() != PICTURE_RECORD_STRIDE {
        return Err(Error::Invalid("picture record has the wrong length".into()));
    }
    if &record[..4] != b"RH2\0" {
        return Err(Error::Unsupported("picture record is not RH2".into()));
    }
    if le_u32(record, 8) != PICTURE_CONTENT_BYTES as u32 {
        return Err(Error::Unsupported(
            "RH2 content length differs from the supported layout".into(),
        ));
    }
    if le_u16(record, 0x54) != PICTURE_SIDE as u16 || le_u16(record, 0x56) != PICTURE_SIDE as u16 {
        return Err(Error::Unsupported(
            "only 128x128 RH2 pictures are supported".into(),
        ));
    }
    let mut palette = [[0u8; 4]; 256];
    for (index, color) in palette.iter_mut().enumerate() {
        *color = rgba5551(le_u16(record, PALETTE_OFFSET + index * 2));
    }
    let mut pixels = vec![0; PICTURE_PIXELS * 4];
    for y in 0..PICTURE_SIDE {
        for x in 0..PICTURE_SIDE {
            let source = ps2_indexed_offset(x, y);
            let source_offset = if source < PIXEL_HALF_BYTES {
                FIRST_PIXELS + source
            } else {
                SECOND_PIXELS + source - PIXEL_HALF_BYTES
            };
            let palette_index = swap_palette_bits(record[source_offset]);
            let destination = (y * PICTURE_SIDE + x) * 4;
            pixels[destination..destination + 4]
                .copy_from_slice(&palette[usize::from(palette_index)]);
        }
    }
    Ok(RgbaImage {
        width: PICTURE_SIDE as u32,
        height: PICTURE_SIDE as u32,
        pixels,
    })
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated fixed layout"),
    )
}
fn rgba5551(word: u16) -> [u8; 4] {
    let expand = |value: u16| ((value * 255) / 31) as u8;
    [
        expand(word & 31),
        expand((word >> 5) & 31),
        expand((word >> 10) & 31),
        if word & 0x8000 == 0 { 0 } else { 255 },
    ]
}
fn swap_palette_bits(index: u8) -> u8 {
    (index & 0xE7) | ((index & 0x08) << 1) | ((index & 0x10) >> 1)
}
fn ps2_indexed_offset(x: usize, y: usize) -> usize {
    let block = (y & !15) * PICTURE_SIDE + (x & !15) * 2;
    let within_row = ((y & !3) / 2 + (y & 1)) & 7;
    let x_shift = (((y + 2) / 4) & 1) * 4;
    let within_column = ((x + x_shift) & 7) * 4;
    let lane = ((y / 2) & 1) + ((x / 4) & 2);
    block + within_row * PICTURE_SIDE * 2 + within_column + lane
}

#[cfg(test)]
mod tests;
