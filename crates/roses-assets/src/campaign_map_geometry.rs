//! Bounded RH0 geometry of the campaign map: the world-object models the
//! story init loads for the map (`0x1E91B0(3, 2, 0)` = MODEL.MRG bank block 35,
//! object slots 34..=65).
//!
//! The block is the executable's bank table entry 35 (EE 0x291CD0 + 4·35 =
//! file 0x136D800, entry 36 = 0x16A3000) and holds 32 RH0 records (ordinals
//! 202..=233 in the archive's aligned-record scan order) interleaved with
//! motion records of the two three-node pieces. Every record uses the
//! story/board packet grammar (one runtime matrix block, initial DMA jump
//! 0x80) and additionally may carry a second, alpha-blended DMA chain: section
//! 3 is a chain without the jump whose geometry references (address kind 5)
//! index section 4, whose strips have PRIM ABE set (GIF word 0x303e4000), and
//! whose TEX0 patches are tagged with address kind 4 (relative to section 3).
//! The main chain may lack a GS register packet (untextured strips only).
//! Raw strips, vertex lanes, materials and GS registers are preserved;
//! placement, lighting, blending, the node sink/rise animation and the camera
//! are the renderer's job and are traced in the private campaign-map research
//! (`story/the project notes), not claimed here. Slot roles live
//! with the map model, not in this decoder.
use crate::story_geometry::{region, wide, word, words, Section, StoryVertex};
use crate::{Error, OpeningTextureRecord, Result, PAL_MODEL_ARCHIVE_SHA256, PROFILE_ID};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Bank table entry (EE 0x291CD0 + 4·35) and its successor, as file offsets.
pub const PAL_CAMPAIGN_MAP_BLOCK: usize = 35;
pub const PAL_CAMPAIGN_MAP_BLOCK_OFFSET: usize = 0x136_D800;
pub const PAL_CAMPAIGN_MAP_BLOCK_END: usize = 0x16A_3000;
/// Object slot of the first record (0x2DE2C0 table); record i is slot 34 + i.
pub const PAL_CAMPAIGN_MAP_FIRST_SLOT: u16 = 34;
pub const PAL_CAMPAIGN_MAP_MODEL_COUNT: usize = 32;
/// Position of the first record among the archive's aligned RH0 records.
pub const PAL_CAMPAIGN_MAP_FIRST_ORDINAL: u16 = 202;

const ARCHIVE_BYTES: usize = 26_949_632;
const MAX_MODEL_BYTES: usize = 1024 * 1024;
const MAX_NODES: usize = 4;
const MAX_STRIPS: usize = 8192;
const MAX_VERTICES: usize = 65536;
const MAX_DRAWS: usize = 128;
const MATRIX_BLOCK_BYTES: usize = 0x70;
/// GIF tag word 1 of a strip: PRIM tri-strip, Gouraud, fog, NREG 3; bit 4
/// (0x08 in the byte) is TME, bit 6 ABE.
const GIF_TEXTURED: u32 = 0x301e_4000;
const GIF_UNTEXTURED: u32 = 0x3016_4000;
const GIF_TEXTURED_BLENDED: u32 = 0x303e_4000;
const GIF_UNTEXTURED_BLENDED: u32 = 0x3036_4000;

#[derive(Clone, Debug, Serialize)]
pub struct CampaignMapGeometry {
    pub schema_version: u32,
    pub profile_id: String,
    pub archive_sha256: String,
    pub block_offset: u64,
    pub block_end: u64,
    /// Bytes of the block outside RH0 records (motion records, padding).
    pub other_record_bytes: u64,
    /// Decoded records in block order (slot 34 + index of the record).
    pub models: Vec<CampaignMapModel>,
    /// Records of the block this decoder does not parse (fingerprinted only).
    pub unsupported: Vec<UnsupportedCampaignMapRecord>,
    pub rendering_validated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CampaignMapModel {
    /// World-object slot the story init loads this record into.
    pub slot: u16,
    pub ordinal: u16,
    pub file_offset: u64,
    pub byte_len: u32,
    pub source_sha256: String,
    pub raw_header_words: Vec<u32>,
    /// Original 64-byte node descriptors (1..=4). Their full grammar is unknown.
    pub raw_node_words: Vec<[u32; 16]>,
    pub node_count: u8,
    /// Node-zero scale: raw int16 positions times this give local units.
    pub node_zero_scale: [f32; 3],
    /// Node-zero words 4..=6 (observed: 16-bit rotation angles, word 5 = Y).
    pub node_zero_rotation_words: [u32; 3],
    pub matrix_slots: u8,
    pub textures: Vec<OpeningTextureRecord>,
    /// Main chain draws first, then the alpha chain's.
    pub draws: Vec<MapDraw>,
    pub strips: Vec<MapStrip>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MapDraw {
    /// Section 3 chain (alpha-blended strips in section 4).
    pub alpha_chain: bool,
    pub dma_tag_offset: u32,
    pub stream_start: u32,
    pub stream_end_exclusive: u32,
    /// None when the chain set no GS registers (untextured draw).
    pub texture_index: Option<u16>,
    /// Three original V4_32 vectors (ambient, diffuse, maximum as observed).
    pub material_words: [u32; 12],
    /// Original seven (value, GS-register) pairs, empty without a register packet.
    pub gs_registers: Vec<[u64; 2]>,
    pub matrix_slot: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct MapStrip {
    pub record_offset: u32,
    pub draw_index: u16,
    /// PRIM TME of the strip's GIF tag.
    pub textured: bool,
    /// PRIM ABE of the strip's GIF tag.
    pub blended: bool,
    pub header_vif_address: u16,
    pub vertex_vif_address: u16,
    pub gif_tag_words: [u32; 4],
    pub vertices: Vec<StoryVertex>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnsupportedCampaignMapRecord {
    pub slot: u16,
    pub ordinal: u16,
    pub file_offset: u64,
    pub byte_len: u32,
    pub source_sha256: String,
    pub reason: String,
}

fn invalid(message: &str) -> Error {
    Error::Invalid(format!("PAL campaign map geometry: {message}"))
}

fn authenticate(archive: &[u8]) -> Result<String> {
    if archive.len() != ARCHIVE_BYTES {
        return Err(invalid("unsupported MODEL.MRG length"));
    }
    let archive_sha256 = format!("{:x}", Sha256::digest(archive));
    if archive_sha256 != PAL_MODEL_ARCHIVE_SHA256 {
        return Err(Error::Unsupported(format!(
            "MODEL.MRG SHA256 {archive_sha256}"
        )));
    }
    Ok(archive_sha256)
}

/// Count the aligned `RH0\0` records that start before `offset`.
fn ordinal_before(archive: &[u8], offset: usize) -> u16 {
    let mut count = 0u16;
    let mut at = 0;
    while at + 4 <= offset.min(archive.len()) {
        if &archive[at..at + 4] == b"RH0\0" {
            count = count.saturating_add(1);
        }
        at += 16;
    }
    count
}

/// Decode the map block after authenticating the complete original MODEL.MRG.
/// `block_offset`/`block_end` are the bank table entries read from the user's
/// executable; they must equal the fingerprinted values.
pub fn decode_pal_campaign_map(
    archive: &[u8],
    block_offset: usize,
    block_end: usize,
) -> Result<CampaignMapGeometry> {
    let archive_sha256 = authenticate(archive)?;
    if block_offset != PAL_CAMPAIGN_MAP_BLOCK_OFFSET || block_end != PAL_CAMPAIGN_MAP_BLOCK_END {
        return Err(invalid(
            "bank table block 35 does not match the fingerprinted map block",
        ));
    }
    if ordinal_before(archive, block_offset) != PAL_CAMPAIGN_MAP_FIRST_ORDINAL {
        return Err(invalid("map block ordinal differs from the fingerprint"));
    }
    let mut models = Vec::new();
    let mut unsupported = Vec::new();
    let mut at = block_offset;
    let mut index = 0usize;
    let mut other_bytes = 0u64;
    while at + 16 <= block_end {
        let head = region(archive, at, 16)?;
        if head.get(..4) != Some(b"RH0\0") {
            // Other record kinds (motion data of the three-node pieces) and
            // the zero padding after the last record are skipped, not decoded.
            other_bytes += 16;
            at += 16;
            continue;
        }
        let size = word(archive, at + 12)? as usize;
        if !(0xb0..=MAX_MODEL_BYTES).contains(&size)
            || !size.is_multiple_of(16)
            || at + size > block_end
            || index >= PAL_CAMPAIGN_MAP_MODEL_COUNT
        {
            return Err(invalid("map record length exceeds the block"));
        }
        let bytes = region(archive, at, size)?;
        let slot = PAL_CAMPAIGN_MAP_FIRST_SLOT + index as u16;
        let ordinal = PAL_CAMPAIGN_MAP_FIRST_ORDINAL + index as u16;
        match parse_map_record(bytes, at, slot, ordinal) {
            Ok(model) => models.push(model),
            Err(error) => unsupported.push(UnsupportedCampaignMapRecord {
                slot,
                ordinal,
                file_offset: at as u64,
                byte_len: size as u32,
                source_sha256: format!("{:x}", Sha256::digest(bytes)),
                reason: error.to_string(),
            }),
        }
        at += size;
        index += 1;
    }
    if index != PAL_CAMPAIGN_MAP_MODEL_COUNT {
        return Err(invalid("map block does not hold 32 records"));
    }
    Ok(CampaignMapGeometry {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        archive_sha256,
        block_offset: block_offset as u64,
        block_end: block_end as u64,
        other_record_bytes: other_bytes,
        models,
        unsupported,
        rendering_validated: false,
    })
}

/// Decode every RH0 record of another MODEL.MRG bank block with the same
/// grammar (e.g. the battle arena: block 22, the shared stage piece, and
/// block 23 + terrain, the terrain's stage). `block_offset`/`block_end` are
/// the bank table entries (EE 0x291CD0 + 4 * block) read from the user's
/// executable; records the grammar does not cover are fingerprinted in
/// `unsupported`. Model slots count from `first_slot`.
pub fn decode_pal_model_bank(
    archive: &[u8],
    block_offset: usize,
    block_end: usize,
    first_slot: u16,
) -> Result<CampaignMapGeometry> {
    let archive_sha256 = authenticate(archive)?;
    if block_end <= block_offset || block_end > archive.len() || !block_offset.is_multiple_of(16) {
        return Err(invalid("bank block outside the archive"));
    }
    let first_ordinal = ordinal_before(archive, block_offset);
    let mut models = Vec::new();
    let mut unsupported = Vec::new();
    let mut at = block_offset;
    let mut index = 0usize;
    let mut other_bytes = 0u64;
    while at + 16 <= block_end {
        let head = region(archive, at, 16)?;
        if head.get(..4) != Some(b"RH0 ") {
            other_bytes += 16;
            at += 16;
            continue;
        }
        let size = word(archive, at + 12)? as usize;
        if !(0xb0..=MAX_MODEL_BYTES).contains(&size)
            || !size.is_multiple_of(16)
            || at + size > block_end
            || index >= 64
        {
            return Err(invalid("bank record length exceeds the block"));
        }
        let bytes = region(archive, at, size)?;
        let slot = first_slot + index as u16;
        let ordinal = first_ordinal + index as u16;
        match parse_map_record(bytes, at, slot, ordinal) {
            Ok(model) => models.push(model),
            Err(error) => unsupported.push(UnsupportedCampaignMapRecord {
                slot,
                ordinal,
                file_offset: at as u64,
                byte_len: size as u32,
                source_sha256: format!("{:x}", Sha256::digest(bytes)),
                reason: error.to_string(),
            }),
        }
        at += size;
        index += 1;
    }
    Ok(CampaignMapGeometry {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        archive_sha256,
        block_offset: block_offset as u64,
        block_end: block_end as u64,
        other_record_bytes: other_bytes,
        models,
        unsupported,
        rendering_validated: false,
    })
}

/// Decode one embedded texture of a map model into lossless indexed form after
/// the same full-archive authentication and structural validation.
pub fn decode_pal_campaign_map_texture(
    archive: &[u8],
    model: &CampaignMapModel,
    texture_index: usize,
) -> Result<crate::OpeningIndexedTexture> {
    authenticate(archive)?;
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

fn parse_map_record(
    bytes: &[u8],
    file_offset: usize,
    slot: u16,
    ordinal: u16,
) -> Result<CampaignMapModel> {
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
    let (dma, geometry, alpha_dma, alpha_geometry, texture_table) = (
        sections[1],
        sections[2],
        sections[3],
        sections[4],
        sections[5],
    );
    let (textures, patches) = parse_map_textures(
        bytes,
        file_offset,
        dma,
        alpha_dma,
        texture_table,
        header_end,
    )?;
    let mut used = BTreeSet::new();
    let (mut draws, matrix_slots) = parse_main_chain(bytes, dma, geometry, &patches, &mut used)?;
    if alpha_dma.end > alpha_dma.start {
        draws.extend(parse_alpha_chain(
            bytes,
            alpha_dma,
            alpha_geometry,
            matrix_slots,
            &patches,
            &mut used,
        )?);
    } else if alpha_geometry.end > alpha_geometry.start {
        return Err(invalid("alpha geometry without an alpha chain"));
    }
    if used.len() != patches.len() {
        return Err(invalid("texture patches outside the parsed chains"));
    }
    if draws.is_empty() {
        return Err(invalid("record has no draws"));
    }
    let mut strips = parse_map_strips(bytes, geometry, &draws, false)?;
    strips.extend(parse_map_strips(bytes, alpha_geometry, &draws, true)?);
    if strips.is_empty() {
        return Err(invalid("empty geometry streams"));
    }
    Ok(CampaignMapModel {
        slot,
        ordinal,
        file_offset: file_offset as u64,
        byte_len: bytes.len() as u32,
        source_sha256: format!("{:x}", Sha256::digest(bytes)),
        raw_header_words: words::<44>(bytes, 0)?.to_vec(),
        node_count: raw_node_words.len() as u8,
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

/// Texture table (section 5): count, then per texture a record offset and a
/// patch list of tagged offsets; kind 2 patches index the main chain (section
/// 1), kind 4 patches the alpha chain (section 3). Each patch must address the
/// TEX0 slot of a GS register packet.
fn parse_map_textures(
    bytes: &[u8],
    file_offset: usize,
    dma: Section,
    alpha_dma: Section,
    table: Section,
    header_end: usize,
) -> Result<(Vec<OpeningTextureRecord>, BTreeMap<usize, u16>)> {
    let count = word(bytes, table.start)? as usize;
    let refs = word(bytes, table.start + 4)? as usize;
    if !(1..=16).contains(&count)
        || refs != 16 + count * 16
        || !table.contains(table.start + refs, count * 8)
    {
        return Err(invalid("invalid texture table"));
    }
    let mut textures = Vec::new();
    let mut patches = BTreeMap::new();
    let mut next_texture = header_end;
    for index in 0..count {
        let offset = word(bytes, table.start + 16 + index * 16)? as usize;
        if offset != next_texture {
            return Err(invalid("embedded textures are not contiguous"));
        }
        let len = word(bytes, offset + 8)? as usize;
        let record = crate::opening_textures::inspect_record(
            region(bytes, offset, len)?,
            index,
            file_offset + offset,
        )?;
        textures.push(record);
        next_texture = offset + len;
        let n = word(bytes, table.start + refs + index * 8)? as usize;
        let start = word(bytes, table.start + refs + index * 8 + 4)? as usize;
        if !(1..=64).contains(&n)
            || start < refs + count * 8
            || start > table.end - table.start
            || !table.contains(table.start + start, n * 4)
        {
            return Err(invalid("texture patch list exceeds table"));
        }
        for j in 0..n {
            let tagged = word(bytes, table.start + start + j * 4)?;
            let relative = (tagged & 0x0fff_ffff) as usize;
            let (section, at) = match tagged >> 28 {
                2 => (dma, dma.start + relative),
                4 => (alpha_dma, alpha_dma.start + relative),
                _ => return Err(invalid("unsupported TEX0 patch section")),
            };
            if !section.contains(at, 16)
                || wide(bytes, at + 8)? != 6
                || patches.insert(at, index as u16).is_some()
            {
                return Err(invalid("invalid or duplicate TEX0 patch"));
            }
        }
    }
    if next_texture != bytes.len() {
        return Err(invalid("unexpected trailing RH0 texture data"));
    }
    Ok((textures, patches))
}

/// Shared chain walker: `chain` holds the DMA tags, `geometry` the strips it
/// references with address kind `geometry_kind`. `slots` is the number of
/// runtime matrix blocks (matrix references are decoded to their slot).
#[allow(clippy::too_many_arguments)]
fn walk_chain(
    bytes: &[u8],
    chain: Section,
    start: usize,
    geometry: Section,
    geometry_kind: u32,
    slots: usize,
    alpha_chain: bool,
    patches: &BTreeMap<usize, u16>,
    used: &mut BTreeSet<usize>,
) -> Result<Vec<MapDraw>> {
    let mut at = start;
    let mut visited = BTreeSet::new();
    let mut texture = None;
    let mut material = None;
    let mut matrix = None;
    let mut registers = Vec::new();
    let mut draws = Vec::new();
    let mut next_geometry = geometry.start;
    while at < chain.end {
        if !chain.contains(at, 16)
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
                if address != 0 || !chain.contains(payload, qwords * 16) {
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
                        used.insert(patch);
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
                if qwords == 7 && vif0 == 0x1000_0000 && vif1 == 0x6c07_0004 {
                    if relative < 0x10
                        || !(relative - 0x10).is_multiple_of(MATRIX_BLOCK_BYTES)
                        || (relative - 0x10) / MATRIX_BLOCK_BYTES >= slots
                    {
                        return Err(invalid("matrix reference outside the runtime blocks"));
                    }
                    matrix = Some(((relative - 0x10) / MATRIX_BLOCK_BYTES) as u8);
                } else if address >> 28 == geometry_kind && vif0 == 0 && vif1 == 0 && qwords > 0 {
                    let start = geometry.start + relative;
                    let len = qwords * 16;
                    if start != next_geometry
                        || !geometry.contains(start, len)
                        || draws.len() >= MAX_DRAWS
                    {
                        return Err(invalid("DMA draw ranges overlap or exceed geometry"));
                    }
                    next_geometry = start + len;
                    draws.push(MapDraw {
                        alpha_chain,
                        dma_tag_offset: at as u32,
                        stream_start: start as u32,
                        stream_end_exclusive: next_geometry as u32,
                        texture_index: texture,
                        material_words: material
                            .ok_or_else(|| invalid("draw has no material vectors"))?,
                        gs_registers: registers.clone(),
                        matrix_slot: matrix
                            .ok_or_else(|| invalid("draw has no matrix reference"))?,
                    });
                } else {
                    return Err(invalid("unsupported DMA reference"));
                }
                at += 16;
            }
            _ => return Err(invalid("unsupported DMA tag")),
        }
    }
    if next_geometry != geometry.end {
        return Err(invalid("draws do not cover the geometry section"));
    }
    Ok(draws)
}

fn parse_main_chain(
    bytes: &[u8],
    dma: Section,
    geometry: Section,
    patches: &BTreeMap<usize, u16>,
    used: &mut BTreeSet<usize>,
) -> Result<(Vec<MapDraw>, u8)> {
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
                && dma.contains(dma.start, jump) =>
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
    let draws = walk_chain(
        bytes,
        dma,
        dma.start + jump,
        geometry,
        3,
        slots,
        false,
        patches,
        used,
    )?;
    Ok((draws, slots as u8))
}

fn parse_alpha_chain(
    bytes: &[u8],
    chain: Section,
    geometry: Section,
    slots: u8,
    patches: &BTreeMap<usize, u16>,
    used: &mut BTreeSet<usize>,
) -> Result<Vec<MapDraw>> {
    walk_chain(
        bytes,
        chain,
        chain.start,
        geometry,
        5,
        usize::from(slots),
        true,
        patches,
        used,
    )
}

fn parse_map_strips(
    bytes: &[u8],
    geometry: Section,
    draws: &[MapDraw],
    alpha_chain: bool,
) -> Result<Vec<MapStrip>> {
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
        let (textured, blended) = match gif[1] {
            GIF_TEXTURED => (true, false),
            GIF_UNTEXTURED => (false, false),
            GIF_TEXTURED_BLENDED => (true, true),
            GIF_UNTEXTURED_BLENDED => (false, true),
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
            .position(|d| {
                d.alpha_chain == alpha_chain
                    && at >= d.stream_start as usize
                    && end <= d.stream_end_exclusive as usize
            })
            .ok_or_else(|| invalid("strip crosses a draw boundary"))?;
        // An untextured strip (PRIM TME off) may sit in a draw that set a
        // texture (the battle stages); a textured strip needs one.
        if textured && draws[draw].texture_index.is_none() {
            return Err(invalid("strip texture mapping disagrees with its draw"));
        }
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
        strips.push(MapStrip {
            record_offset: at as u32,
            draw_index: draw as u16,
            textured,
            blended,
            header_vif_address: command as u16,
            vertex_vif_address: next as u16,
            gif_tag_words: gif,
            vertices: vs,
        });
        vertices += n;
        at = end;
    }
    Ok(strips)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinal_counts_only_aligned_records_before_the_offset() {
        let mut bytes = vec![0u8; 256];
        bytes[0..4].copy_from_slice(b"RH0\0");
        bytes[16..20].copy_from_slice(b"RH0\0");
        // Unaligned marker is not a record.
        bytes[36..40].copy_from_slice(b"RH0\0");
        bytes[128..132].copy_from_slice(b"RH0\0");
        assert_eq!(ordinal_before(&bytes, 128), 2);
        assert_eq!(ordinal_before(&bytes, 256), 3);
        assert_eq!(ordinal_before(&bytes, 0), 0);
    }

    #[test]
    fn wrong_archive_or_block_is_rejected() {
        assert!(decode_pal_campaign_map(&[0; 64], 0, 64).is_err());
        assert!(parse_map_record(&[0; 0xb0], 0, 34, 202).is_err());
    }

    // Generated strip packets with invented values; no retail fixture.
    fn put(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn strip(gif1: u32) -> Vec<u8> {
        let mut b = vec![0; 80];
        for (i, v) in [0x6c01_8000, 3, gif1, 0x412, 4, 0x6909_8001]
            .into_iter()
            .enumerate()
        {
            put(&mut b, i * 4, v);
        }
        for (i, v) in [
            0i16, 0, 1, 0, 32767, 0, -20, 0, -20, 4096, 0, 1, 0, 32767, 0, 20, 0, -20, 0, 4096, 1,
            0, 32767, 0, 0, 0, 20,
        ]
        .into_iter()
        .enumerate()
        {
            b[24 + i * 2..26 + i * 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
    fn draw(alpha: bool, textured: bool, start: u32, end: u32) -> MapDraw {
        MapDraw {
            alpha_chain: alpha,
            dma_tag_offset: 0,
            stream_start: start,
            stream_end_exclusive: end,
            texture_index: textured.then_some(0),
            material_words: [0; 12],
            gs_registers: Vec::new(),
            matrix_slot: 0,
        }
    }

    #[test]
    fn strips_record_texture_and_blend_flags_per_chain() {
        let mut b = strip(GIF_TEXTURED_BLENDED);
        b.extend(strip(GIF_UNTEXTURED_BLENDED));
        let section = Section {
            start: 0,
            end: b.len(),
        };
        let draws = [
            draw(true, true, 0, 80),
            draw(true, false, 80, 160),
            draw(false, true, 0, 160),
        ];
        let strips = parse_map_strips(&b, section, &draws, true).unwrap();
        assert_eq!(strips.len(), 2);
        assert!(strips[0].textured && strips[0].blended && strips[0].draw_index == 0);
        assert!(!strips[1].textured && strips[1].blended && strips[1].draw_index == 1);
        assert_eq!(strips[0].vertices[2].position_raw, [0, 0, 20]);
        // A textured strip inside an untextured draw is refused.
        let mismatch = [draw(true, false, 0, 160)];
        assert!(parse_map_strips(&b, section, &mismatch, true).is_err());
        // Unknown GIF word.
        let bad = strip(0x3000_0000);
        let s = Section {
            start: 0,
            end: bad.len(),
        };
        assert!(parse_map_strips(&bad, s, &[draw(false, true, 0, 80)], false).is_err());
    }

    #[test]
    #[ignore = "Requires the user's original MODEL.MRG (ROSES_TEST_MODEL_PATH)"]
    fn private_map_block_decodes_thirty_one_records() {
        let path = std::env::var_os("ROSES_TEST_MODEL_PATH").expect("ROSES_TEST_MODEL_PATH");
        let archive = std::fs::read(path).unwrap();
        let geometry = decode_pal_campaign_map(
            &archive,
            PAL_CAMPAIGN_MAP_BLOCK_OFFSET,
            PAL_CAMPAIGN_MAP_BLOCK_END,
        )
        .unwrap();
        assert_eq!(geometry.models.len(), 31);
        assert_eq!(geometry.unsupported.len(), 1);
        assert_eq!(geometry.unsupported[0].slot, 65);
        assert_eq!(geometry.models[0].slot, 34);
        assert_eq!(geometry.models[0].file_offset, 0x136_D800);
        // Tile 34: two textured main draws and one alpha draw (private check).
        let tile = &geometry.models[0];
        assert_eq!(tile.draws.iter().filter(|d| d.alpha_chain).count(), 1);
        assert_eq!(tile.draws.len(), 3);
        assert!(tile.strips.iter().any(|s| s.blended));
        let three: Vec<u16> = geometry
            .models
            .iter()
            .filter(|m| m.node_count == 3)
            .map(|m| m.slot)
            .collect();
        assert_eq!(three, [40, 41]);
        for model in &geometry.models {
            for index in 0..model.textures.len() {
                decode_pal_campaign_map_texture(&archive, model, index).unwrap();
            }
        }
    }
}
