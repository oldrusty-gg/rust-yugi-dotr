//! Bounded source geometry for the eight PAL opening-scene RH0 records.
//! This preserves VIF/GIF strips and raw vertex lanes, rather than executing PS2
//! programs. Camera, texture-coordinate conversion, clipping, culling, lighting
//! and animation are not reconstructed by this importer.
use crate::{opening_textures, Error, OpeningTextureRecord, Result, RgbaImage, PROFILE_ID};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PAL_MODEL_ARCHIVE_SHA256: &str =
    "a3b31c2db9a96ddf1c3e49faa8060eebb32094e7164930eac249fc6400721441";
const ARCHIVE_BYTES: usize = 26_949_632;
const MODEL_OFFSETS: [usize; 8] = [
    0x16aa000, 0x1700ef0, 0x1717ef0, 0x17263b0, 0x173b580, 0x1769200, 0x178b9f0, 0x17959b0,
];
const MAX_MODEL_BYTES: usize = 1024 * 1024;
const MAX_STRIPS: usize = 4096;
const MAX_VERTICES: usize = 32768;

#[derive(Clone, Debug, Serialize)]
pub struct StoryGeometry {
    pub schema_version: u32,
    pub profile_id: String,
    pub archive_sha256: String,
    pub models: Vec<StoryModel>,
    pub rendering_validated: bool,
    pub gameplay_validated: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct StoryModel {
    /// World-object slot in the observed original scene, not a universal model ID.
    pub observed_object_slot: u16,
    pub file_offset: u64,
    pub byte_len: u32,
    pub source_sha256: String,
    pub raw_header_words: Vec<u32>,
    /// Original 64-byte node descriptors. Their full transform grammar is unknown.
    pub raw_node_words: Vec<[u32; 16]>,
    /// Observed first three float fields of node zero. Multiplication with raw
    /// positions gives coherent local geometry; this is not a world transform.
    pub node_zero_scale: [f32; 3],
    pub textures: Vec<OpeningTextureRecord>,
    pub draws: Vec<StoryDraw>,
    pub strips: Vec<StoryStrip>,
}
#[derive(Clone, Debug, Serialize)]
pub struct StoryDraw {
    /// 0 = first DMA chain (section 1, geometry section 2); 1 = the second
    /// chain (sections 3/4: TEX0 patches tagged 4, references tagged 4/5),
    /// which carries blended draws (ALPHA 0x44) in the story worlds' RH0s.
    #[serde(default)]
    pub pass: u8,
    pub dma_tag_offset: u32,
    pub stream_start: u32,
    pub stream_end_exclusive: u32,
    pub texture_index: u16,
    /// Three original V4_32 vectors; retain their bits without assuming lighting.
    pub material_words: [u32; 12],
    /// Original seven (value, GS-register) pairs active for this draw.
    pub gs_registers: Vec<[u64; 2]>,
}
#[derive(Clone, Debug, Serialize)]
pub struct StoryStrip {
    pub record_offset: u32,
    pub draw_index: u16,
    pub header_vif_address: u16,
    pub vertex_vif_address: u16,
    /// Encodes triangle-strip PRIM and ST/RGBAQ/XYZF2 register order. Preserved
    /// unmodified, including EOP; final VU clipping/culling is not simulated.
    pub gif_tag_words: [u32; 4],
    pub vertices: Vec<StoryVertex>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StoryVertex {
    /// First V3_16 vector. Do not assume normalized UV or the third lane's meaning.
    pub texture_raw: [i16; 3],
    pub normal_raw: [i16; 3],
    pub position_raw: [i16; 3],
}

/// Inspect only the supported original MODEL.MRG, using eight source records
/// independently matched to the original scene's loaded RH0 data.
pub fn decode_pal_story_geometry(archive: &[u8]) -> Result<StoryGeometry> {
    if archive.len() != ARCHIVE_BYTES {
        return Err(invalid("unsupported MODEL.MRG length"));
    }
    let hash = format!("{:x}", Sha256::digest(archive));
    if hash != PAL_MODEL_ARCHIVE_SHA256 {
        return Err(Error::Unsupported(format!("MODEL.MRG SHA256 {hash}")));
    }
    let mut models = Vec::new();
    for (index, offset) in MODEL_OFFSETS.into_iter().enumerate() {
        let size = word(archive, offset + 12)? as usize;
        if !(0xb0..=MAX_MODEL_BYTES).contains(&size) {
            return Err(invalid("RH0 record length exceeds bounds"));
        }
        let bytes = region(archive, offset, size)?;
        models.push(parse_model(bytes, offset, 34 + index as u16)?);
    }
    Ok(StoryGeometry {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        archive_sha256: hash,
        models,
        rendering_validated: false,
        gameplay_validated: false,
    })
}

/// Decode an embedded RH2 texture after authenticating and validating the complete
/// supported archive. Indices are local to one observed object slot (34..41).
pub fn decode_pal_story_model_texture(
    archive: &[u8],
    object_slot: u16,
    texture_index: usize,
) -> Result<RgbaImage> {
    let geometry = decode_pal_story_geometry(archive)?;
    let record = geometry
        .models
        .iter()
        .find(|m| m.observed_object_slot == object_slot)
        .and_then(|m| m.textures.get(texture_index))
        .ok_or_else(|| invalid("scene model or texture index is out of range"))?;
    opening_textures::decode_record(archive, record)
}

fn invalid(message: &str) -> Error {
    Error::Invalid(format!("PAL story geometry: {message}"))
}
pub(crate) fn region(bytes: &[u8], start: usize, count: usize) -> Result<&[u8]> {
    let end = start
        .checked_add(count)
        .ok_or_else(|| invalid("range overflow"))?;
    bytes
        .get(start..end)
        .ok_or_else(|| invalid("range exceeds record"))
}
pub(crate) fn word(bytes: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        region(bytes, at, 4)?.try_into().unwrap(),
    ))
}
pub(crate) fn wide(bytes: &[u8], at: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        region(bytes, at, 8)?.try_into().unwrap(),
    ))
}
pub(crate) fn words<const N: usize>(bytes: &[u8], at: usize) -> Result<[u32; N]> {
    let data = region(bytes, at, N * 4)?;
    Ok(std::array::from_fn(|i| {
        u32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap())
    }))
}
#[derive(Clone, Copy)]
pub(crate) struct Section {
    pub(crate) start: usize,
    pub(crate) end: usize,
}
impl Section {
    pub(crate) fn read(bytes: &[u8], at: usize) -> Result<Self> {
        let start = word(bytes, at)? as usize;
        let size = word(bytes, at + 4)? as usize;
        region(bytes, start, size)?;
        if start < 0xb0 || !start.is_multiple_of(16) || !size.is_multiple_of(16) {
            return Err(invalid("unaligned or overlapping header section"));
        }
        Ok(Self {
            start,
            end: start + size,
        })
    }
    pub(crate) fn contains(self, start: usize, length: usize) -> bool {
        start >= self.start && start.checked_add(length).is_some_and(|end| end <= self.end)
    }
}

/// Parse one RH0 record (also used by the opening-scene derivation for the
/// records of a world pack located through the executable's tables).
pub(crate) fn parse_model(bytes: &[u8], file_offset: usize, slot: u16) -> Result<StoryModel> {
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
    if !matches!(nodes.end - nodes.start, 64 | 128) {
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
    let dma2 = sections[3];
    let geometry2 = sections[4];
    let texture_table = sections[5];
    let second = (dma2.end > dma2.start).then_some(dma2);
    let (textures, patches) =
        parse_texture_passes(bytes, file_offset, dma, second, texture_table, header_end)?;
    let mut used = BTreeSet::new();
    let mut draws = parse_draws(bytes, dma, geometry, &patches, 0, &mut used)?;
    let mut strips = if draws.is_empty() {
        Vec::new()
    } else {
        parse_strips(bytes, geometry, &draws)?
    };
    if second.is_some() {
        let pass1 = parse_draws(bytes, dma2, geometry2, &patches, 1, &mut used)?;
        let base = draws.len() as u16;
        let mut extra = parse_strips(bytes, geometry2, &pass1)?;
        for strip in &mut extra {
            strip.draw_index += base;
        }
        draws.extend(pass1);
        strips.extend(extra);
    }
    if draws.is_empty() || used.len() != patches.len() {
        return Err(invalid("draws do not cover geometry or texture patches"));
    }
    Ok(StoryModel {
        observed_object_slot: slot,
        file_offset: file_offset as u64,
        byte_len: bytes.len() as u32,
        source_sha256: format!("{:x}", Sha256::digest(bytes)),
        raw_header_words: words::<44>(bytes, 0)?.to_vec(),
        raw_node_words,
        node_zero_scale: scale,
        textures,
        draws,
        strips,
    })
}

pub(crate) fn parse_textures(
    bytes: &[u8],
    file_offset: usize,
    dma: Section,
    table: Section,
    header_end: usize,
) -> Result<(Vec<OpeningTextureRecord>, BTreeMap<usize, u16>)> {
    parse_texture_passes(bytes, file_offset, dma, None, table, header_end)
}

/// Texture table with TEX0 patches into the first (tag 2) and, when the
/// record has one, the second DMA chain (tag 4). Keys are record offsets.
pub(crate) fn parse_texture_passes(
    bytes: &[u8],
    file_offset: usize,
    dma: Section,
    dma2: Option<Section>,
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
        let record = opening_textures::inspect_record(
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
            let section = match (tagged >> 28, dma2) {
                (2, _) => dma,
                (4, Some(second)) => second,
                _ => return Err(invalid("invalid or duplicate TEX0 patch")),
            };
            let at = section.start + (tagged & 0x0fff_ffff) as usize;
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

fn parse_draws(
    bytes: &[u8],
    dma: Section,
    geometry: Section,
    patches: &BTreeMap<usize, u16>,
    pass: u8,
    used_patches: &mut BTreeSet<usize>,
) -> Result<Vec<StoryDraw>> {
    let mut at = dma.start;
    let mut visited = BTreeSet::new();
    // Pass 0 references the matrix block (tag 2) and geometry (tag 3); pass 1
    // tags 4/5 (the same layout as the monster models' blended chain).
    let (matrix_tag, geometry_tag) = if pass == 0 { (2, 3) } else { (4, 5) };
    let mut texture = None;
    let mut material = None;
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
            2 if pass == 0 => {
                if at != dma.start
                    || qwords != 0
                    || address != 0x2000_0080
                    || vif0 != 0x0300_0080
                    || vif1 != 0x0200_01c0
                {
                    return Err(invalid("unsupported initial DMA jump"));
                }
                at = dma.start + 0x80;
            }
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
                match address >> 28 {
                    t if t == matrix_tag
                        && qwords == 7
                        && address & 0x0fff_ffff == 0x10
                        && vif0 == 0x1000_0000
                        && vif1 == 0x6c07_0004 => {}
                    t if t == geometry_tag && vif0 == 0 && vif1 == 0 && qwords > 0 => {
                        let start = geometry.start + (address & 0x0fff_ffff) as usize;
                        let len = qwords * 16;
                        if start != next_geometry
                            || !geometry.contains(start, len)
                            || draws.len() >= 128
                        {
                            return Err(invalid("DMA draw ranges overlap or exceed geometry"));
                        }
                        next_geometry = start + len;
                        draws.push(StoryDraw {
                            pass,
                            dma_tag_offset: at as u32,
                            stream_start: start as u32,
                            stream_end_exclusive: next_geometry as u32,
                            texture_index: texture.ok_or_else(|| invalid("draw has no texture"))?,
                            material_words: material
                                .ok_or_else(|| invalid("draw has no material vectors"))?,
                            gs_registers: registers.clone(),
                        });
                    }
                    _ => return Err(invalid("unsupported DMA reference")),
                }
                at += 16;
            }
            _ => return Err(invalid("unsupported DMA tag")),
        }
    }
    if next_geometry != geometry.end {
        return Err(invalid("draws do not cover geometry or texture patches"));
    }
    Ok(draws)
}

fn parse_strips(bytes: &[u8], geometry: Section, draws: &[StoryDraw]) -> Result<Vec<StoryStrip>> {
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
        if gif[0] & !0xffff != 0
            // PRIM 0x3C (textured, Gouraud, fogged strip) or 0x7C (the same
            // with alpha blending, second-chain draws).
            || !matches!(gif[1], 0x301e_4000 | 0x303e_4000)
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
        strips.push(StoryStrip {
            record_offset: at as u32,
            draw_index: draw as u16,
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
