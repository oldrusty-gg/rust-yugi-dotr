//! Bounded duel-board RH0 geometry from the PAL `DATA/MODEL.MRG`: the ten
//! terrain tiles, the arena records and the 42 leader-kind window records.
//!
//! This is the story RH0 packet grammar (`story_geometry`) generalised for the
//! board: records may carry up to four nodes and reserve one runtime matrix
//! block per drawable node (each draw records the block it uses), and strips
//! may be untextured (PRIM TME off). Raw strips, vertex lanes, materials and GS
//! register values are preserved. Placement, lighting, camera, clipping and
//! the runtime node animation are the renderer's responsibility and are not
//! claimed here. Locations were matched to the loaded EE copies of a private
//! duel savestate (research notes, not data embedded here).
use crate::story_geometry::{parse_textures, region, wide, word, words, Section, StoryVertex};
use crate::{Error, OpeningTextureRecord, Result, PAL_MODEL_ARCHIVE_SHA256, PROFILE_ID};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Terrain block loaded as one unit (0x1F800 bytes to EE 0x102D000).
pub const PAL_DUEL_TERRAIN_BLOCK_OFFSET: usize = 0x17F_0000;
pub const PAL_DUEL_TERRAIN_BLOCK_BYTES: usize = 0x1_F800;
/// Terrain code `c` (0 forest .. 9 crush) is RH0 record #245 + c.
pub const PAL_DUEL_TERRAIN_FIRST_ORDINAL: u16 = 245;
pub const PAL_DUEL_TERRAIN_COUNT: usize = 10;
/// Arena records #0..#2 (object slots 0..2), read from the start of the archive.
pub const PAL_DUEL_ARENA_OFFSETS: [usize; 3] = [0, 0x2_9590, 0x4_DFC0];
/// Arena records #3/#4 (end pieces) use a geometry packet form this decoder
/// does not support; they are fingerprinted and reported, never guessed.
pub const PAL_DUEL_ARENA_UNSUPPORTED_OFFSETS: [usize; 2] = [0x5_7080, 0x5_A7A0];
/// Window record #(255 + 21 * side + leader card kind), one per 0xA000 slot.
pub const PAL_DUEL_WINDOW_FIRST_OFFSET: usize = 0x180_F800;
pub const PAL_DUEL_WINDOW_STRIDE: usize = 0xA000;
pub const PAL_DUEL_WINDOW_FIRST_ORDINAL: u16 = 255;
pub const PAL_DUEL_WINDOW_KINDS: usize = 21;
pub const PAL_DUEL_WINDOW_COUNT: usize = 2 * PAL_DUEL_WINDOW_KINDS;

const ARCHIVE_BYTES: usize = 26_949_632;
const MAX_MODEL_BYTES: usize = 1024 * 1024;
const MAX_NODES: usize = 4;
const MAX_STRIPS: usize = 4096;
const MAX_VERTICES: usize = 32768;
const MAX_DRAWS: usize = 128;
/// Size of one runtime matrix block reserved at the head of the DMA section.
const MATRIX_BLOCK_BYTES: usize = 0x70;
/// GIF tag word 1 of a strip: PRIM tri-strip, Gouraud, fog, NREG 3, with and
/// without texture mapping (TME).
const GIF_TEXTURED: u32 = 0x301e_4000;
const GIF_UNTEXTURED: u32 = 0x3016_4000;

#[derive(Clone, Debug, Serialize)]
pub struct DuelBoardGeometry {
    pub schema_version: u32,
    pub profile_id: String,
    pub archive_sha256: String,
    /// Terrain codes 0..=9 in order.
    pub terrain: Vec<BoardModel>,
    /// Arena object slots 0..=2 in order.
    pub arena: Vec<BoardModel>,
    /// Window records in ordinal order: index = 21 * side + leader card kind.
    pub windows: Vec<BoardModel>,
    pub unsupported: Vec<UnsupportedBoardRecord>,
    pub rendering_validated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum BoardRole {
    Terrain { code: u8 },
    Arena { slot: u8 },
    Window { side: u8, leader_kind: u8 },
}

#[derive(Clone, Debug, Serialize)]
pub struct UnsupportedBoardRecord {
    pub ordinal: u16,
    pub file_offset: u64,
    pub byte_len: u32,
    pub source_sha256: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardModel {
    #[serde(flatten)]
    pub role: BoardRole,
    /// Position among the archive's 16-byte aligned RH0 records.
    pub ordinal: u16,
    pub file_offset: u64,
    pub byte_len: u32,
    pub source_sha256: String,
    pub raw_header_words: Vec<u32>,
    /// Original 64-byte node descriptors (1..=4). Their full grammar is unknown.
    pub raw_node_words: Vec<[u32; 16]>,
    /// Node-zero scale: raw int16 positions times this give local units.
    pub node_zero_scale: [f32; 3],
    /// Node-zero words 4..=6 (observed: 16-bit rotation angles, word 5 = Y).
    pub node_zero_rotation_words: [u32; 3],
    /// Runtime matrix blocks reserved in the DMA section.
    pub matrix_slots: u8,
    pub textures: Vec<OpeningTextureRecord>,
    pub draws: Vec<BoardDraw>,
    pub strips: Vec<BoardStrip>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardDraw {
    pub dma_tag_offset: u32,
    pub stream_start: u32,
    pub stream_end_exclusive: u32,
    pub texture_index: u16,
    /// Three original V4_32 vectors (ambient, diffuse, maximum as observed).
    pub material_words: [u32; 12],
    /// Original seven (value, GS-register) pairs active for this draw.
    pub gs_registers: Vec<[u64; 2]>,
    /// Runtime matrix block this draw references (0-based).
    pub matrix_slot: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardStrip {
    pub record_offset: u32,
    pub draw_index: u16,
    /// PRIM TME of the strip's GIF tag; untextured strips use vertex colour only.
    pub textured: bool,
    pub header_vif_address: u16,
    pub vertex_vif_address: u16,
    pub gif_tag_words: [u32; 4],
    pub vertices: Vec<StoryVertex>,
}

fn invalid(message: &str) -> Error {
    Error::Invalid(format!("PAL duel board geometry: {message}"))
}

/// Decode the supported board records after authenticating the complete
/// original MODEL.MRG. Offsets are fixed for this fingerprint only.
pub fn decode_pal_duel_board(archive: &[u8]) -> Result<DuelBoardGeometry> {
    if archive.len() != ARCHIVE_BYTES {
        return Err(invalid("unsupported MODEL.MRG length"));
    }
    let archive_sha256 = format!("{:x}", Sha256::digest(archive));
    if archive_sha256 != PAL_MODEL_ARCHIVE_SHA256 {
        return Err(Error::Unsupported(format!(
            "MODEL.MRG SHA256 {archive_sha256}"
        )));
    }
    let mut terrain = Vec::new();
    let mut at = PAL_DUEL_TERRAIN_BLOCK_OFFSET;
    for code in 0..PAL_DUEL_TERRAIN_COUNT {
        let bytes = record(archive, at)?;
        if at + bytes.len() > PAL_DUEL_TERRAIN_BLOCK_OFFSET + PAL_DUEL_TERRAIN_BLOCK_BYTES {
            return Err(invalid("terrain record exceeds the loaded terrain block"));
        }
        terrain.push(parse_board_model(
            bytes,
            at,
            PAL_DUEL_TERRAIN_FIRST_ORDINAL + code as u16,
            BoardRole::Terrain { code: code as u8 },
        )?);
        at += bytes.len();
    }
    let mut arena = Vec::new();
    for (slot, offset) in PAL_DUEL_ARENA_OFFSETS.into_iter().enumerate() {
        let bytes = record(archive, offset)?;
        let next = PAL_DUEL_ARENA_OFFSETS
            .get(slot + 1)
            .copied()
            .unwrap_or(PAL_DUEL_ARENA_UNSUPPORTED_OFFSETS[0]);
        if offset + bytes.len() > next {
            return Err(invalid("arena records overlap"));
        }
        arena.push(parse_board_model(
            bytes,
            offset,
            slot as u16,
            BoardRole::Arena { slot: slot as u8 },
        )?);
    }
    let mut unsupported = Vec::new();
    for (index, offset) in PAL_DUEL_ARENA_UNSUPPORTED_OFFSETS.into_iter().enumerate() {
        let bytes = record(archive, offset)?;
        unsupported.push(UnsupportedBoardRecord {
            ordinal: 3 + index as u16,
            file_offset: offset as u64,
            byte_len: bytes.len() as u32,
            source_sha256: format!("{:x}", Sha256::digest(bytes)),
            reason: "arena end piece: its geometry stream uses a packet form this decoder does not parse".into(),
        });
    }
    let mut windows = Vec::new();
    for index in 0..PAL_DUEL_WINDOW_COUNT {
        let offset = PAL_DUEL_WINDOW_FIRST_OFFSET + index * PAL_DUEL_WINDOW_STRIDE;
        let bytes = record(archive, offset)?;
        if bytes.len() > PAL_DUEL_WINDOW_STRIDE {
            return Err(invalid("window record exceeds its slot"));
        }
        windows.push(parse_board_model(
            bytes,
            offset,
            PAL_DUEL_WINDOW_FIRST_ORDINAL + index as u16,
            BoardRole::Window {
                side: (index / PAL_DUEL_WINDOW_KINDS) as u8,
                leader_kind: (index % PAL_DUEL_WINDOW_KINDS) as u8,
            },
        )?);
    }
    Ok(DuelBoardGeometry {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        archive_sha256,
        terrain,
        arena,
        windows,
        unsupported,
        rendering_validated: false,
    })
}

/// Decode one embedded texture of a board model into lossless indexed form
/// after the same full-archive authentication and structural validation.
pub fn decode_pal_duel_board_texture(
    archive: &[u8],
    model: &BoardModel,
    texture_index: usize,
) -> Result<crate::OpeningIndexedTexture> {
    if archive.len() != ARCHIVE_BYTES
        || format!("{:x}", Sha256::digest(archive)) != PAL_MODEL_ARCHIVE_SHA256
    {
        return Err(Error::Unsupported("MODEL.MRG fingerprint".into()));
    }
    let start = model.file_offset as usize;
    let bytes = region(archive, start, model.byte_len as usize)?;
    if format!("{:x}", Sha256::digest(bytes)) != model.source_sha256 {
        return Err(invalid("model record does not match its fingerprint"));
    }
    let texture = model
        .textures
        .get(texture_index)
        .ok_or_else(|| invalid("texture index out of range"))?;
    crate::opening_textures::decode_indexed_record(archive, texture)
}

fn record(archive: &[u8], offset: usize) -> Result<&[u8]> {
    if !offset.is_multiple_of(16) || region(archive, offset, 16)?.get(..4) != Some(b"RH0\0") {
        return Err(invalid("expected an aligned RH0 record"));
    }
    let size = word(archive, offset + 12)? as usize;
    if !(0xb0..=MAX_MODEL_BYTES).contains(&size) || !size.is_multiple_of(16) {
        return Err(invalid("RH0 record length exceeds bounds"));
    }
    region(archive, offset, size)
}

pub(crate) fn parse_board_model(
    bytes: &[u8],
    file_offset: usize,
    ordinal: u16,
    role: BoardRole,
) -> Result<BoardModel> {
    if !(0xb0..=MAX_MODEL_BYTES).contains(&bytes.len())
        || bytes.get(..4) != Some(b"RH0\0")
        || word(bytes, 12)? as usize != bytes.len()
        || !matches!(word(bytes, 4)?, 0x8000_0000 | 0x8000_0001)
    {
        return Err(invalid("unsupported RH0 header or size"));
    }
    let header_end = word(bytes, 8)? as usize;
    let mut sections = Vec::new();
    for at in (0x10..0x60).step_by(8) {
        sections.push(Section::read(bytes, at)?);
    }
    if sections[0].start != 0xb0
        || sections.last().unwrap().end != header_end
        || sections.windows(2).any(|pair| pair[0].end != pair[1].start)
    {
        return Err(invalid("RH0 sections are not contiguous"));
    }
    let nodes = sections[0];
    let node_bytes = nodes.end - nodes.start;
    if node_bytes == 0 || !node_bytes.is_multiple_of(64) || node_bytes / 64 > MAX_NODES {
        return Err(invalid("unsupported node count"));
    }
    let raw_node_words: Vec<[u32; 16]> = (nodes.start..nodes.end)
        .step_by(64)
        .map(|at| words(bytes, at))
        .collect::<Result<_>>()?;
    let scale = std::array::from_fn(|i| f32::from_bits(raw_node_words[0][i]));
    if scale
        .iter()
        .any(|n| !n.is_finite() || *n <= 0.0 || *n > 1024.0)
    {
        return Err(invalid("invalid node-zero scale"));
    }
    let dma = sections[1];
    let geometry = sections[2];
    let texture_table = sections[5];
    let (textures, patches) = parse_textures(bytes, file_offset, dma, texture_table, header_end)?;
    let (draws, matrix_slots) = parse_board_draws(bytes, dma, geometry, &patches)?;
    let strips = parse_board_strips(bytes, geometry, &draws)?;
    Ok(BoardModel {
        role,
        ordinal,
        file_offset: file_offset as u64,
        byte_len: bytes.len() as u32,
        source_sha256: format!("{:x}", Sha256::digest(bytes)),
        raw_header_words: words::<44>(bytes, 0)?.to_vec(),
        node_zero_rotation_words: [
            raw_node_words[0][4],
            raw_node_words[0][5],
            raw_node_words[0][6],
        ],
        raw_node_words,
        node_zero_scale: scale,
        matrix_slots,
        textures,
        draws,
        strips,
    })
}

pub(crate) fn parse_board_draws(
    bytes: &[u8],
    dma: Section,
    geometry: Section,
    patches: &BTreeMap<usize, u16>,
) -> Result<(Vec<BoardDraw>, u8)> {
    // Initial jump over the zeroed matrix blocks the runtime fills in.
    let [tag, address, vif0, vif1] = words::<4>(bytes, dma.start)?;
    let jump = (address & 0x0fff_ffff) as usize;
    let slots = jump.checked_sub(0x10).map(|n| n / MATRIX_BLOCK_BYTES);
    let slots = match slots {
        Some(n)
            if tag == 0x2000_0000
                && address >> 28 == 2
                && vif0 == 0x0300_0080
                && vif1 == 0x0200_01c0
                && (1..=MAX_NODES).contains(&n)
                && jump == 0x10 + n * MATRIX_BLOCK_BYTES
                && dma.contains(dma.start, jump + 16) =>
        {
            n
        }
        _ => return Err(invalid("unsupported initial DMA jump")),
    };
    if region(bytes, dma.start + 16, jump - 16)?
        .iter()
        .any(|b| *b != 0)
    {
        return Err(invalid("runtime matrix blocks are not zero on disc"));
    }
    let mut at = dma.start + jump;
    let mut visited = BTreeSet::new();
    let mut used_patches = BTreeSet::new();
    let mut texture = None;
    let mut material = None;
    let mut matrix = None;
    let mut registers = Vec::new();
    let mut draws = Vec::new();
    let mut next_geometry = geometry.start;
    while at < dma.end {
        if !dma.contains(at, 16)
            || !at.is_multiple_of(16)
            || !visited.insert(at)
            || visited.len() > 1024
        {
            return Err(invalid("invalid or cycling DMA chain"));
        }
        let [tag, address, vif0, vif1] = words::<4>(bytes, at)?;
        let qwords = (tag & 65535) as usize;
        if tag & 0x8fff_0000 != 0 {
            return Err(invalid("unsupported DMA flags"));
        }
        match (tag >> 28) & 7 {
            1 => {
                let payload = at + 16;
                if address != 0 || !dma.contains(payload, qwords * 16) {
                    return Err(invalid("DMA payload exceeds section"));
                }
                match (qwords, vif0, vif1) {
                    (8, 0x1100_0000, 0x5100_0008) => {
                        if wide(bytes, payload)? != 0x1000_0000_0000_8007
                            || wide(bytes, payload + 8)? != 14
                        {
                            return Err(invalid("unsupported GS register packet"));
                        }
                        registers = (0..7)
                            .map(|i| {
                                Ok([
                                    wide(bytes, payload + 16 + i * 16)?,
                                    wide(bytes, payload + 24 + i * 16)?,
                                ])
                            })
                            .collect::<Result<_>>()?;
                        let expected = [8, 0x34, 0x36, 0x47, 0x42, 6, 0x14];
                        if registers.iter().map(|r| r[1]).ne(expected) {
                            return Err(invalid("unsupported GS register order"));
                        }
                        let patch = payload + 16 + 5 * 16;
                        texture = Some(
                            *patches
                                .get(&patch)
                                .ok_or_else(|| invalid("unmapped texture selection"))?,
                        );
                        used_patches.insert(patch);
                    }
                    (3, 0x1000_0000, 0x6c03_003c) => {
                        material = Some(words::<12>(bytes, payload)?);
                    }
                    _ => return Err(invalid("unsupported DMA inline packet")),
                }
                at = payload + qwords * 16;
            }
            3 => {
                let relative = (address & 0x0fff_ffff) as usize;
                match address >> 28 {
                    2 if qwords == 7
                        && vif0 == 0x1000_0000
                        && vif1 == 0x6c07_0004
                        && relative >= 0x10
                        && (relative - 0x10).is_multiple_of(MATRIX_BLOCK_BYTES)
                        && (relative - 0x10) / MATRIX_BLOCK_BYTES < slots =>
                    {
                        matrix = Some(((relative - 0x10) / MATRIX_BLOCK_BYTES) as u8);
                    }
                    3 if vif0 == 0 && vif1 == 0 && qwords > 0 => {
                        let start = geometry.start + relative;
                        let len = qwords * 16;
                        if start != next_geometry
                            || !geometry.contains(start, len)
                            || draws.len() >= MAX_DRAWS
                        {
                            return Err(invalid("DMA draw ranges overlap or exceed geometry"));
                        }
                        next_geometry = start + len;
                        draws.push(BoardDraw {
                            dma_tag_offset: at as u32,
                            stream_start: start as u32,
                            stream_end_exclusive: next_geometry as u32,
                            texture_index: texture.ok_or_else(|| invalid("draw has no texture"))?,
                            material_words: material
                                .ok_or_else(|| invalid("draw has no material vectors"))?,
                            gs_registers: registers.clone(),
                            matrix_slot: matrix
                                .ok_or_else(|| invalid("draw has no matrix reference"))?,
                        });
                    }
                    _ => return Err(invalid("unsupported DMA reference")),
                }
                at += 16;
            }
            _ => return Err(invalid("unsupported DMA tag")),
        }
    }
    if draws.is_empty() || next_geometry != geometry.end || used_patches.len() != patches.len() {
        return Err(invalid("draws do not cover geometry or texture patches"));
    }
    Ok((draws, slots as u8))
}

pub(crate) fn parse_board_strips(
    bytes: &[u8],
    geometry: Section,
    draws: &[BoardDraw],
) -> Result<Vec<BoardStrip>> {
    let mut at = geometry.start;
    let mut strips = Vec::new();
    let mut vertices = 0usize;
    while at < geometry.end {
        let command = word(bytes, at)?;
        if command == 0 || command == 0x1500_0000 {
            at += 4;
            continue;
        }
        if command & 0xffff_0000 != 0x6c01_0000
            || command & 0x0000_fc00 != 0x8000
            || !geometry.contains(at, 24)
        {
            return Err(invalid("unsupported geometry VIF command"));
        }
        let gif = words::<4>(bytes, at + 4)?;
        let n = (gif[0] & 0x7fff) as usize;
        let next = word(bytes, at + 20)?;
        let textured = match gif[1] {
            GIF_TEXTURED => true,
            GIF_UNTEXTURED => false,
            _ => return Err(invalid("unsupported strip GIF tag")),
        };
        if gif[0] & !0xffff != 0
            || gif[2..] != [0x412, 4]
            || !(3..=85).contains(&n)
            || next >> 24 != 0x69
            || ((next >> 16) & 255) as usize != n * 3
            || next & 0xffff != (command & 0xffff) + 1
        {
            return Err(invalid("inconsistent triangle-strip packet"));
        }
        let end = (at + 24 + n * 18 + 3) & !3;
        if end > geometry.end || strips.len() >= MAX_STRIPS || vertices + n > MAX_VERTICES {
            return Err(invalid("geometry packet exceeds supported bounds"));
        }
        let draw = draws
            .iter()
            .position(|d| at >= d.stream_start as usize && end <= d.stream_end_exclusive as usize)
            .ok_or_else(|| invalid("strip crosses a draw boundary"))?;
        let mut vs = Vec::with_capacity(n);
        for i in 0..n {
            let data = region(bytes, at + 24 + i * 18, 18)?;
            let values: [i16; 9] = std::array::from_fn(|i| {
                i16::from_le_bytes(data[i * 2..i * 2 + 2].try_into().unwrap())
            });
            vs.push(StoryVertex {
                texture_raw: values[..3].try_into().unwrap(),
                normal_raw: values[3..6].try_into().unwrap(),
                position_raw: values[6..].try_into().unwrap(),
            });
        }
        if region(bytes, at + 24 + n * 18, end - (at + 24 + n * 18))?
            .iter()
            .any(|b| *b != 0)
        {
            return Err(invalid("nonzero VIF padding"));
        }
        strips.push(BoardStrip {
            record_offset: at as u32,
            draw_index: draw as u16,
            textured,
            header_vif_address: command as u16,
            vertex_vif_address: next as u16,
            gif_tag_words: gif,
            vertices: vs,
        });
        vertices += n;
        at = end;
    }
    if strips.is_empty() {
        return Err(invalid("empty geometry stream"));
    }
    Ok(strips)
}

#[cfg(test)]
mod tests;
