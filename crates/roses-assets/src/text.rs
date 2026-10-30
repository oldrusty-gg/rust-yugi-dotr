//! Bounded PAL English text and verified dialogue/name-entry bitmap-font cells.
//! Compression is expanded without recursion, in dependency order. General
//! messages retain unknown codes; card names still accept only their verified
//! subset. Font extraction does not establish placement or scene sequencing.

use super::{Error, Result, RgbaImage, CARD_COUNT, PROFILE_ID};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const PAL_ENGLISH_MESSAGE_SHA256: &str =
    "c88ee92a21c1cd36baa7e5d56dd41386a7fa906cd1796d7537c62217c1a2c1e3";
pub const MESSAGE_COUNT: usize = 4111;
pub const CARD_NAME_FIRST_MESSAGE: usize = 323;
const MESSAGE_BYTES: usize = 0x45180;
const OFFSET_TABLE_START: usize = 0x100;
const TOKEN_POOL_START: usize = 0x413C;
const MAX_RAW_TOKENS: usize = 1024;
const MAX_EXPANDED_TOKENS: usize = 4096;
const MAX_TOTAL_EXPANDED_TOKENS: usize = 1_000_000;
const MAX_NAME_CHARACTERS: usize = 256;
const MAX_SELECTED_MESSAGES: usize = 256;
const TOKEN_POOL_END: usize = 0x3239A;
// The original runtime font pointer is two bytes after the final message.
const FONT_FILE_OFFSET: usize = 0x3239C;
const FONT_SIDE: usize = 22;
const FONT_GLYPH_BYTES: usize = FONT_SIDE * FONT_SIDE / 2;
const FONT_GLYPH_COUNT: usize = 164;
const NAME_FONT_GLYPH_COUNT: usize = 317;
const FONT_COLUMNS: usize = 16;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageToken {
    /// A visually identified cell in the first original font bank. Character
    /// transcription is optional: the cell remains renderable without it.
    Glyph {
        code: u16,
        atlas_index: u16,
        character: Option<char>,
    },
    /// No rendering or control semantics are assigned to this preserved code.
    Unknown { code: u16 },
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct MessageLine {
    pub tokens: Vec<MessageToken>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PalMessage {
    pub message_id: u16,
    /// Expanded original codes, including all observed 0x2000 line separators.
    pub expanded_codes: Vec<u16>,
    pub lines: Vec<MessageLine>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PalMessages {
    pub schema_version: u32,
    pub profile_id: String,
    pub message_sha256: String,
    pub language: String,
    /// Requested order is retained; this is not an inferred scene order.
    pub messages: Vec<PalMessage>,
    pub gameplay_validated: bool,
}

/// Leaf opcode identity, preserved through every compressed reference.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct MessageSource {
    pub message_id: u16,
    pub raw_token_offset: u16,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageTemplateToken {
    /// Includes newline 0x2000 and all unknown direct codes without guessing.
    Direct { code: u16, source: MessageSource },
    /// Only the original mutable name words in message3 become variables.
    /// No substitution is inferred from a placeholder glyph's numeric value.
    PlayerNameSlot { slot: u8, source: MessageSource },
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PalMessageTemplate {
    pub message_id: u16,
    pub tokens: Vec<MessageTemplateToken>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PalMessageTemplates {
    pub schema_version: u32,
    pub profile_id: String,
    pub message_sha256: String,
    pub language: String,
    pub messages: Vec<PalMessageTemplate>,
    pub gameplay_validated: bool,
}

/// Decode selected original messages while preserving mutable player-name leaf
/// slots. The original renderer measures reference spans in consumed raw words,
/// stops at the referenced stream's end bit, and then resumes its caller.
/// Templates follow those source-proven rules, including nested references.
/// Selected name spans must begin with the first name slot (or the separate
/// first-character alias); arbitrary suffix references are rejected because a
/// prior end flag is not encountered when decoding starts inside that suffix.
pub fn parse_pal_message_templates(
    message_file: &[u8],
    message_ids: &[u16],
) -> Result<PalMessageTemplates> {
    validate_message_selection(message_ids)?;
    let message_sha256 = verified_message_hash(message_file)?;
    let offsets = container_offsets(message_file)?;
    let decoded =
        decode_template_messages(&message_file[TOKEN_POOL_START..TOKEN_POOL_END], &offsets)?;
    let messages = message_ids
        .iter()
        .map(|&id| structure_template(id, &decoded[usize::from(id)]))
        .collect::<Result<Vec<_>>>()?;
    Ok(PalMessageTemplates {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        message_sha256,
        language: "en".into(),
        messages,
        gameplay_validated: false,
    })
}

/// Resolve a template using 1..12 committed original narrow glyph codes.
/// Unused name slots emit nothing: the original end flag stops the source
/// reference after the last selected character. Selected blank glyphs remain;
/// neither whitespace trimming nor Unicode conversion is performed.
pub fn resolve_pal_message_template(
    template: &PalMessageTemplate,
    stored_codes: &[u16],
) -> Result<PalMessage> {
    if stored_codes.is_empty()
        || stored_codes.len() > 12
        || stored_codes.iter().any(|code| glyph_index(*code).is_none())
    {
        return Err(Error::Invalid(
            "player name requires 1..12 direct supported narrow glyphs".into(),
        ));
    }
    validate_template(template)?;
    let mut codes = Vec::with_capacity(template.tokens.len());
    for token in &template.tokens {
        match token {
            MessageTemplateToken::Direct { code, source } => {
                if code & 0xC000 != 0 || player_name_slot(*source).is_some() {
                    return Err(Error::Invalid(
                        "direct template token has flags or mutable name provenance".into(),
                    ));
                }
                codes.push(*code);
            }
            MessageTemplateToken::PlayerNameSlot { slot, source } => {
                if player_name_slot(*source) != Some(*slot) {
                    return Err(Error::Invalid(
                        "name slot does not match original mutable source identity".into(),
                    ));
                }
                if let Some(&code) = stored_codes.get(usize::from(*slot)) {
                    codes.push(code);
                }
            }
        }
    }
    Ok(structure_message(template.message_id, &codes))
}

fn structure_template(
    message_id: u16,
    expansion: &TemplateExpansion,
) -> Result<PalMessageTemplate> {
    let tokens = expansion
        .tokens
        .iter()
        .map(|atom| {
            if let Some(slot) = player_name_slot(atom.source) {
                MessageTemplateToken::PlayerNameSlot {
                    slot,
                    source: atom.source,
                }
            } else {
                MessageTemplateToken::Direct {
                    code: atom.code,
                    source: atom.source,
                }
            }
        })
        .collect();
    let template = PalMessageTemplate { message_id, tokens };
    validate_template(&template)?;
    Ok(template)
}

fn validate_template(template: &PalMessageTemplate) -> Result<()> {
    if usize::from(template.message_id) >= MESSAGE_COUNT
        || template.tokens.len() > MAX_EXPANDED_TOKENS
    {
        return Err(Error::Invalid(
            "message template exceeds supported bounds".into(),
        ));
    }
    let mut prior_slot = None;
    for token in &template.tokens {
        let source = match token {
            MessageTemplateToken::Direct { source, .. } => {
                prior_slot = None;
                source
            }
            MessageTemplateToken::PlayerNameSlot { slot, source } => {
                if player_name_slot(*source) != Some(*slot) {
                    return Err(Error::Invalid(
                        "name slot does not match original mutable source identity".into(),
                    ));
                }
                if source.raw_token_offset == 0 {
                    prior_slot = None;
                } else {
                    if *slot != 0 && prior_slot != Some(*slot - 1) {
                        return Err(Error::Unsupported("name references beginning inside a name require explicit source-span semantics".into()));
                    }
                    prior_slot = Some(*slot);
                }
                source
            }
        };
        if usize::from(source.message_id) >= MESSAGE_COUNT
            || usize::from(source.raw_token_offset) >= MAX_RAW_TOKENS
        {
            return Err(Error::Invalid(
                "template source exceeds message bounds".into(),
            ));
        }
    }
    Ok(())
}

fn player_name_slot(source: MessageSource) -> Option<u8> {
    if source.message_id != 3 {
        return None;
    }
    match source.raw_token_offset {
        0 => Some(0),
        2..=13 => Some((source.raw_token_offset - 2) as u8),
        _ => None,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageFontMetadata {
    pub schema_version: u32,
    pub profile_id: String,
    pub message_sha256: String,
    pub language: String,
    pub file_offset: usize,
    pub glyph_count: usize,
    pub glyph_width: u32,
    pub glyph_height: u32,
    pub glyph_bytes: usize,
    pub atlas_columns: usize,
    /// Bytes after the requested verified cells remain uninterpreted.
    pub uninterpreted_trailer_offset: usize,
    pub uninterpreted_trailer_bytes: usize,
    pub gameplay_validated: bool,
}

#[derive(Clone, Debug)]
pub struct MessageFont {
    pub metadata: MessageFontMetadata,
    /// White RGB with the original 4-bit intensity expanded to 8-bit alpha.
    /// The original first-bank renderer copies the left 12 pixels of each
    /// stored 22-pixel row and advances 12 raw coordinates; line pitch is 22.
    /// Name-entry's wide glyphs use all22 columns of their separate cells.
    /// Scene tint, coordinate transform and placement belong to the caller.
    pub image: RgbaImage,
}

impl MessageFont {
    /// Original code to the stored atlas cell [x,y,22,22], excluding newline.
    /// Render the left 12 columns for the supported first-bank codes.
    /// Unknown/alternative-bank codes are never masked into supported cells.
    pub fn glyph_rect(&self, code: u16) -> Option<[u32; 4]> {
        glyph_index(code).map(|index| {
            [
                (index % FONT_COLUMNS * FONT_SIDE) as u32,
                (index / FONT_COLUMNS * FONT_SIDE) as u32,
                FONT_SIDE as u32,
                FONT_SIDE as u32,
            ]
        })
    }

    /// The two original name-entry grids use unflagged, full-width cells164..316.
    /// Available only from `decode_pal_name_font`; never mask unknown codes.
    pub fn name_glyph_rect(&self, code: u16) -> Option<[u32; 4]> {
        if !(0x00A4..=0x013C).contains(&code) || usize::from(code) >= self.metadata.glyph_count {
            return None;
        }
        let index = usize::from(code);
        Some([
            (index % FONT_COLUMNS * FONT_SIDE) as u32,
            (index / FONT_COLUMNS * FONT_SIDE) as u32,
            FONT_SIDE as u32,
            FONT_SIDE as u32,
        ])
    }
}

/// Expand at most 256 distinct selected messages from the exact supported
/// English file. Only 0x2000 is treated as a line separator. Unknown codes are
/// retained rather than silently stripped or replaced with guessed text.
pub fn parse_pal_messages(message_file: &[u8], message_ids: &[u16]) -> Result<PalMessages> {
    validate_message_selection(message_ids)?;
    let message_sha256 = verified_message_hash(message_file)?;
    let decoded = parse_container(message_file)?;
    let messages = message_ids
        .iter()
        .map(|&id| structure_message(id, &decoded[usize::from(id)].glyphs))
        .collect();
    Ok(PalMessages {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        message_sha256,
        language: "en".into(),
        messages,
        gameplay_validated: false,
    })
}

/// Decode the first 164 22x22 cells in the message-file bitmap font. The
/// supported range covers ordinary English dialogue; alternate-bank glyphs
/// remain unsupported. No copyrighted pixels are embedded in the program.
pub fn decode_pal_message_font(message_file: &[u8]) -> Result<MessageFont> {
    decode_message_font(message_file, FONT_GLYPH_COUNT)
}

/// Decode both the ordinary narrow dialogue cells and the full-width cells
/// referenced by the original two name-entry pages. No retail pixels are embedded.
pub fn decode_pal_name_font(message_file: &[u8]) -> Result<MessageFont> {
    decode_message_font(message_file, NAME_FONT_GLYPH_COUNT)
}

fn decode_message_font(message_file: &[u8], count: usize) -> Result<MessageFont> {
    let message_sha256 = verified_message_hash(message_file)?;
    let end = FONT_FILE_OFFSET + count * FONT_GLYPH_BYTES;
    let source = message_file
        .get(FONT_FILE_OFFSET..end)
        .ok_or_else(|| Error::Invalid("message font extends beyond file".into()))?;
    let image = decode_font_bank(source, count)?;
    Ok(MessageFont {
        metadata: MessageFontMetadata {
            schema_version: 1,
            profile_id: PROFILE_ID.into(),
            message_sha256,
            language: "en".into(),
            file_offset: FONT_FILE_OFFSET,
            glyph_count: count,
            glyph_width: FONT_SIDE as u32,
            glyph_height: FONT_SIDE as u32,
            glyph_bytes: FONT_GLYPH_BYTES,
            atlas_columns: FONT_COLUMNS,
            uninterpreted_trailer_offset: end,
            uninterpreted_trailer_bytes: message_file.len() - end,
            gameplay_validated: false,
        },
        image,
    })
}

fn verified_message_hash(bytes: &[u8]) -> Result<String> {
    if bytes.len() > MESSAGE_BYTES {
        return Err(Error::Invalid(
            "message file exceeds supported byte limit".into(),
        ));
    }
    let hash = format!("{:x}", Sha256::digest(bytes));
    if hash != PAL_ENGLISH_MESSAGE_SHA256 {
        return Err(Error::UnsupportedMessageFile {
            actual_sha256: hash,
        });
    }
    Ok(hash)
}

fn validate_message_selection(ids: &[u16]) -> Result<()> {
    if ids.is_empty() || ids.len() > MAX_SELECTED_MESSAGES {
        return Err(Error::Invalid("select between 1 and 256 messages".into()));
    }
    let mut seen = vec![false; MESSAGE_COUNT];
    for &id in ids {
        let prior = seen
            .get_mut(usize::from(id))
            .ok_or_else(|| Error::Invalid("selected message ID exceeds table".into()))?;
        if *prior {
            return Err(Error::Invalid("duplicate selected message ID".into()));
        }
        *prior = true;
    }
    Ok(())
}

fn glyph_index(code: u16) -> Option<usize> {
    (0x2001..=0x20A3)
        .contains(&code)
        .then(|| usize::from(code - 0x2000))
}

fn structure_message(message_id: u16, codes: &[u16]) -> PalMessage {
    let lines = codes
        .split(|&code| code == 0x2000)
        .map(|line| MessageLine {
            tokens: line
                .iter()
                .map(|&code| match glyph_index(code) {
                    Some(index) => MessageToken::Glyph {
                        code,
                        atlas_index: index as u16,
                        character: recognized_character(code),
                    },
                    None => MessageToken::Unknown { code },
                })
                .collect(),
        })
        .collect();
    PalMessage {
        message_id,
        expanded_codes: codes.to_vec(),
        lines,
    }
}

fn recognized_character(code: u16) -> Option<char> {
    // The name mapping is already checked against all 854 names. Additional
    // punctuation here is directly visible in the decoded original font.
    match code {
        0x204C => Some('('),
        0x204D => Some(')'),
        0x2075 => Some('!'),
        0x2076 => Some('?'),
        0x209C => Some(':'),
        _ => decode_name(&[code]).ok().and_then(|s| s.chars().next()),
    }
}

fn decode_font_bank(source: &[u8], count: usize) -> Result<RgbaImage> {
    if count == 0 || count > NAME_FONT_GLYPH_COUNT || source.len() != count * FONT_GLYPH_BYTES {
        return Err(Error::Invalid(
            "font bank count or byte span is unsupported".into(),
        ));
    }
    let width = FONT_COLUMNS * FONT_SIDE;
    let height = count.div_ceil(FONT_COLUMNS) * FONT_SIDE;
    let mut pixels = vec![0; width * height * 4];
    for index in 0..count {
        for y in 0..FONT_SIDE {
            for x in 0..FONT_SIDE {
                let packed = source[index * FONT_GLYPH_BYTES + (y * FONT_SIDE + x) / 2];
                let alpha = ((packed >> ((x % 2) * 4)) & 0xF) * 17;
                let out = ((index / FONT_COLUMNS * FONT_SIDE + y) * width
                    + index % FONT_COLUMNS * FONT_SIDE
                    + x)
                    * 4;
                pixels[out..out + 4].copy_from_slice(&[255, 255, 255, alpha]);
            }
        }
    }
    Ok(RgbaImage {
        width: width as u32,
        height: height as u32,
        pixels,
    })
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CardName {
    pub id: u16,
    pub message_id: u16,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CardNames {
    pub schema_version: u32,
    pub profile_id: String,
    pub message_sha256: String,
    pub language: String,
    pub names: Vec<CardName>,
    /// Text extraction does not validate any gameplay rule.
    pub gameplay_validated: bool,
}

/// Decode the 854 card names in the exact supported `MSG/MSG_E.DAT`.
/// The caller should read this file from the same verified PAL ISO as the
/// catalogue. General text rendering and other languages are unsupported.
pub fn parse_pal_card_names(message_file: &[u8]) -> Result<CardNames> {
    let message_sha256 = format!("{:x}", Sha256::digest(message_file));
    if message_sha256 != PAL_ENGLISH_MESSAGE_SHA256 {
        return Err(Error::UnsupportedMessageFile {
            actual_sha256: message_sha256,
        });
    }
    let messages = parse_container(message_file)?;
    let mut names = Vec::with_capacity(CARD_COUNT);
    for id in 0..CARD_COUNT {
        let message_id = CARD_NAME_FIRST_MESSAGE + id;
        let message = messages
            .get(message_id)
            .ok_or_else(|| Error::Invalid("card-name message lies outside offset table".into()))?;
        names.push(CardName {
            id: id as u16,
            message_id: message_id as u16,
            name: decode_name(&message.glyphs)?,
        });
    }
    Ok(CardNames {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        message_sha256,
        language: "en".into(),
        names,
        gameplay_validated: false,
    })
}

fn parse_container(bytes: &[u8]) -> Result<Vec<ExpandedMessage>> {
    let offsets = container_offsets(bytes)?;
    decode_messages(&bytes[TOKEN_POOL_START..TOKEN_POOL_END], &offsets)
}

fn container_offsets(bytes: &[u8]) -> Result<Vec<u32>> {
    if bytes.len() != MESSAGE_BYTES || bytes.get(..4) != Some(b"MWo3".as_slice()) {
        return Err(Error::Invalid(
            "unexpected PAL message container size or magic".into(),
        ));
    }
    for (offset, expected) in [
        (4, 1),
        (8, 0x3B0000),
        (12, 0xC0),
        (16, 0x45080),
        (20, 0),
        (24, 0x3F5180),
        (28, 0x3F5180),
    ] {
        if read_u32(bytes, offset)? != expected {
            return Err(Error::Unsupported(
                "PAL message header differs from the supported layout".into(),
            ));
        }
    }
    if bytes.get(0x20..0x2A) != Some(b"msg_e.dat\0".as_slice()) {
        return Err(Error::Unsupported(
            "message container language label is unsupported".into(),
        ));
    }
    let table_end = OFFSET_TABLE_START + MESSAGE_COUNT * 4;
    if table_end != TOKEN_POOL_START {
        return Err(Error::Invalid(
            "inconsistent message profile constants".into(),
        ));
    }
    let mut offsets = Vec::with_capacity(MESSAGE_COUNT);
    for index in 0..MESSAGE_COUNT {
        offsets.push(read_u32(bytes, OFFSET_TABLE_START + index * 4)?);
    }
    // The last encoded stream ends two bytes before the first bitmap bank.
    // Do not allow malformed stream lengths to consume bitmap bytes as tokens.
    Ok(offsets)
}

#[derive(Clone)]
struct TemplateAtom {
    code: u16,
    source: MessageSource,
}
struct TemplateExpansion {
    tokens: Vec<TemplateAtom>,
    /// Includes the terminal boundary after the last consumed raw word.
    raw_boundaries: Vec<Option<usize>>,
}

fn decode_template_messages(pool: &[u8], offsets: &[u32]) -> Result<Vec<TemplateExpansion>> {
    if offsets.is_empty()
        || offsets.len() > MESSAGE_COUNT
        || offsets[0] != 0
        || offsets.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(Error::Invalid("invalid template message offsets".into()));
    }
    let mut starts = Vec::with_capacity(offsets.len());
    for &offset in offsets {
        starts.push(
            usize::try_from(offset)
                .ok()
                .and_then(|offset| offset.checked_mul(2))
                .filter(|start| start.checked_add(2).is_some_and(|end| end <= pool.len()))
                .ok_or_else(|| Error::Invalid("template offset exceeds token pool".into()))?,
        );
    }
    let mut messages: Vec<TemplateExpansion> = Vec::with_capacity(starts.len());
    let mut total = 0usize;
    for (message_id, &start) in starts.iter().enumerate() {
        let end = starts.get(message_id + 1).copied().unwrap_or(pool.len());
        let raw = read_raw_message(&pool[start..end], message_id + 1 < starts.len())?;
        let mut decoded = TemplateExpansion {
            tokens: Vec::new(),
            raw_boundaries: vec![None; raw.len() + 1],
        };
        let mut position = 0;
        while position < raw.len() {
            let raw_token_offset = position;
            decoded.raw_boundaries[position] = Some(decoded.tokens.len());
            let code = raw[position] & 0x7FFF;
            position += 1;
            if code & 0x4000 != 0 {
                let operand = *raw.get(position).ok_or_else(|| {
                    Error::Invalid("template reference is missing operand".into())
                })?;
                position += 1;
                if operand & 0x4000 == 0 {
                    return Err(Error::Invalid(
                        "template reference operand marker is absent".into(),
                    ));
                }
                let source_id = usize::from(operand & 0x3FFF);
                let count = usize::from(code & 0x3F);
                let source_start = usize::from((code & 0x1FFF) >> 6);
                if source_id >= message_id || count == 0 {
                    return Err(Error::Invalid(
                        "template reference is forward, cyclic or empty".into(),
                    ));
                }
                let source = &messages[source_id];
                let source_length = source.raw_boundaries.len() - 1;
                if source_start >= source_length {
                    return Err(Error::Invalid(
                        "template reference starts beyond source".into(),
                    ));
                }
                // The original recursive decoder stops on a source end flag,
                // even if fewer raw words than the reference count remain.
                let source_end = (source_start + count).min(source_length);
                let from = source.raw_boundaries[source_start].ok_or_else(|| {
                    Error::Invalid("template reference begins inside operand pair".into())
                })?;
                let to = source.raw_boundaries[source_end].ok_or_else(|| {
                    Error::Invalid("template reference ends inside operand pair".into())
                })?;
                let selected = &source.tokens[from..to];
                if decoded.tokens.len() + selected.len() > MAX_EXPANDED_TOKENS {
                    return Err(Error::Invalid(
                        "template expansion exceeds per-message bound".into(),
                    ));
                }
                decoded.tokens.extend_from_slice(selected);
            } else {
                if decoded.tokens.len() >= MAX_EXPANDED_TOKENS {
                    return Err(Error::Invalid(
                        "template expansion exceeds per-message bound".into(),
                    ));
                }
                decoded.tokens.push(TemplateAtom {
                    code,
                    source: MessageSource {
                        message_id: message_id as u16,
                        raw_token_offset: raw_token_offset as u16,
                    },
                });
            }
        }
        decoded.raw_boundaries[raw.len()] = Some(decoded.tokens.len());
        total += decoded.tokens.len();
        if total > MAX_TOTAL_EXPANDED_TOKENS {
            return Err(Error::Invalid(
                "template expansion exceeds total token bound".into(),
            ));
        }
        messages.push(decoded);
    }
    Ok(messages)
}

struct ExpandedMessage {
    glyphs: Vec<u16>,
    /// Only opcode starts have an expanded offset. The second word of a
    /// reference is explicitly None and cannot be targeted by another reference.
    raw_boundaries: Vec<Option<usize>>,
}

fn decode_messages(pool: &[u8], offsets: &[u32]) -> Result<Vec<ExpandedMessage>> {
    if offsets.is_empty() || offsets.len() > MESSAGE_COUNT || offsets[0] != 0 {
        return Err(Error::Invalid(
            "unsupported message count or first offset".into(),
        ));
    }
    if offsets.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::Invalid(
            "message offsets must strictly increase".into(),
        ));
    }
    let mut byte_offsets = Vec::with_capacity(offsets.len());
    for &offset in offsets {
        let offset = usize::try_from(offset)
            .ok()
            .and_then(|value| value.checked_mul(2))
            .filter(|value| value.checked_add(2).is_some_and(|end| end <= pool.len()))
            .ok_or_else(|| Error::Invalid("message token offset exceeds pool".into()))?;
        byte_offsets.push(offset);
    }
    let mut messages: Vec<ExpandedMessage> = Vec::with_capacity(offsets.len());
    let mut total_glyphs = 0usize;
    for (index, &start) in byte_offsets.iter().enumerate() {
        let end = byte_offsets.get(index + 1).copied().unwrap_or(pool.len());
        let raw = read_raw_message(&pool[start..end], index + 1 < offsets.len())?;
        let mut expanded = ExpandedMessage {
            glyphs: Vec::new(),
            raw_boundaries: vec![None; raw.len()],
        };
        let mut position = 0usize;
        while position < raw.len() {
            expanded.raw_boundaries[position] = Some(expanded.glyphs.len());
            let token = raw[position] & 0x7FFF;
            position += 1;
            if token & 0x6000 == 0x4000 {
                let &source_token = raw.get(position).ok_or_else(|| {
                    Error::Invalid("reference is missing its second token".into())
                })?;
                position += 1;
                if source_token & 0x4000 == 0 {
                    return Err(Error::Unsupported(
                        "reference index lacks its observed operand marker".into(),
                    ));
                }
                let source_index = usize::from(source_token & 0x3FFF);
                if source_index >= index {
                    return Err(Error::Invalid(
                        "message reference is cyclic or points forward".into(),
                    ));
                }
                let count = usize::from(token & 0x3F);
                if count == 0 {
                    return Err(Error::Unsupported("zero-length message reference".into()));
                }
                let raw_source_offset = usize::from((token & 0x3FFF) >> 6);
                let source = &messages[source_index];
                let glyph_offset = source
                    .raw_boundaries
                    .get(raw_source_offset)
                    .copied()
                    .flatten()
                    .ok_or_else(|| {
                        Error::Invalid(
                            "reference target is outside a message or inside a reference pair"
                                .into(),
                        )
                    })?;
                let selected = source
                    .glyphs
                    .get(glyph_offset..glyph_offset + count)
                    .ok_or_else(|| {
                        Error::Invalid("reference copy exceeds its expanded source".into())
                    })?;
                if expanded.glyphs.len() + selected.len() > MAX_EXPANDED_TOKENS {
                    return Err(Error::Invalid(
                        "message expansion exceeds its per-message limit".into(),
                    ));
                }
                expanded.glyphs.extend_from_slice(selected);
            } else {
                if expanded.glyphs.len() >= MAX_EXPANDED_TOKENS {
                    return Err(Error::Invalid(
                        "message expansion exceeds its per-message limit".into(),
                    ));
                }
                expanded.glyphs.push(token);
            }
        }
        total_glyphs += expanded.glyphs.len();
        if total_glyphs > MAX_TOTAL_EXPANDED_TOKENS {
            return Err(Error::Invalid(
                "message expansion exceeds the total glyph limit".into(),
            ));
        }
        messages.push(expanded);
    }
    Ok(messages)
}

fn read_raw_message(bytes: &[u8], exact_span: bool) -> Result<Vec<u16>> {
    let mut tokens = Vec::new();
    for pair in bytes.chunks_exact(2) {
        if tokens.len() >= MAX_RAW_TOKENS {
            return Err(Error::Invalid(
                "encoded message exceeds raw-token limit".into(),
            ));
        }
        let token = u16::from_le_bytes([pair[0], pair[1]]);
        tokens.push(token);
        if token & 0x8000 != 0 {
            if exact_span && tokens.len() * 2 != bytes.len() {
                return Err(Error::Invalid(
                    "message terminates before the next indexed message".into(),
                ));
            }
            return Ok(tokens);
        }
    }
    Err(Error::Invalid("message lacks a bounded end token".into()))
}

fn decode_name(glyphs: &[u16]) -> Result<String> {
    if glyphs.is_empty() || glyphs.len() > MAX_NAME_CHARACTERS {
        return Err(Error::Invalid(
            "card name is empty or exceeds its character limit".into(),
        ));
    }
    glyphs
        .iter()
        .map(|&glyph| {
            let character = match glyph {
                0x2001 => ' ',
                0x2003 => ',',
                0x2005..=0x201E => char::from(b'A' + (glyph - 0x2005) as u8),
                0x201F..=0x2038 => char::from(b'a' + (glyph - 0x201F) as u8),
                0x2039..=0x2041 => char::from(b'1' + (glyph - 0x2039) as u8),
                0x2042 => '0',
                0x2043 => '-',
                0x2047 => '#',
                0x204A => '&',
                0x204B => '\'',
                0x2072 => '.',
                _ => {
                    return Err(Error::Unsupported(format!(
                        "card-name glyph or control token 0x{glyph:04X}"
                    )))
                }
            };
            Ok(character)
        })
        .collect()
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Error::Invalid("message header offset overflow".into()))?;
    let raw = bytes
        .get(offset..end)
        .ok_or_else(|| Error::Invalid("truncated message header or offset table".into()))?;
    Ok(u32::from_le_bytes(raw.try_into().expect("length checked")))
}

#[cfg(test)]
mod tests;
