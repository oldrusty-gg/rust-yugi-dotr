//! Bounded PAL `DATA/MONSTER.MRG` unit models (monsters and duel leaders).
//!
//! The archive is 683 slots of 1 MiB; slot N belongs to card id N (leaders
//! load the slot of their card, loader 0x1F15A0 -> 0x1F1710). A slot holds an
//! RH0 model followed by the records the loader chains by RH0 header word 1:
//! bits 0-7 MOT motion records (size at +4), bits 11-18 attack-effect RH2
//! textures (size at +8), bits 19/20 CMOT camera records (size at +8, rounded
//! to 16) and bit 21 the sound-cue table. Bits 8-10 (0x8000-byte blocks and
//! an RH2) are never set in the supported archive and are rejected.
//!
//! The RH0 model: 64-byte skeleton nodes (preorder = motion track order), a
//! first DMA chain (opaque pass) that starts with a NEXT over the runtime
//! matrix blocks (0x10 + 0x70k, zero on disc, filled per frame by 0x2100D0),
//! a second DMA chain (blended pass), GS register packets, material vectors,
//! 7-qword matrix uploads to VU slots 4 + 7k, and VIF geometry batches with
//! rigid (one matrix per strip) or weighted (up to eight bones per vertex,
//! each with its own bone-local position and normal) vertices.
//!
//! Everything is preserved as raw lanes. Items whose meaning is unresolved
//! are kept raw and listed in `unresolved`, never interpreted: the MOT
//! per-node u16 map, CMOT camera paths, texture-table variant words and the
//! RH0 section 9 texture-variant table (0x211700), the rigid UV third lane
//! and the third lane of weighted influences. The decoder never executes
//! packets or follows pointers outside the slot. A slot is structurally
//! validated only; the caller authenticates its source (the CLI verifies the
//! complete ISO and archive fingerprints).
use crate::opening_textures::{self, OpeningIndexedTexture, OpeningPaletteFormat};
use crate::story_geometry::{region, wide, word};
use crate::{require_pal_executable, Error, Result, PROFILE_ID};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PAL_MONSTER_ARCHIVE_ISO_PATH: &str = "DATA/MONSTER.MRG";
pub const PAL_MONSTER_ARCHIVE_BYTES: u64 = 716_177_408;
pub const PAL_MONSTER_ARCHIVE_SHA256: &str =
    "e2ea2e4802b2d8411fb92659caef7b5b6d37227e966c255dca2e496d37b96d2f";
pub const PAL_MONSTER_SLOT_BYTES: usize = 0x10_0000;
pub const PAL_MONSTER_SLOT_COUNT: usize = 683;
/// Quarter-wave sine table read by 0x20ACC0 (1025 f32, EE 0x293430).
pub const PAL_SINE_TABLE_EE_ADDRESS: u32 = 0x0029_3430;
pub const PAL_SINE_TABLE_FILE_OFFSET: usize = 0x19_9DB0;
pub const PAL_SINE_TABLE_ENTRIES: usize = 1025;
/// Duel motion slots: idle (SetMotion 0x1F2A40 everywhere), attack
/// (0x1F1EE0), damage (0x1D1C98, 0x262FC4), destroyed (0x1F2060).
pub const MONSTER_MOTION_IDLE: u8 = 0;
pub const MONSTER_MOTION_ATTACK: u8 = 2;
pub const MONSTER_MOTION_DAMAGE: u8 = 4;
pub const MONSTER_MOTION_DESTROYED: u8 = 5;

const RH0_HEADER_BYTES: usize = 0xB0;
const NODE_BYTES: usize = 64;
const MAX_NODES: usize = 512;
const MATRIX_BLOCK_BYTES: usize = 0x70;
const MAX_DRAWS: usize = 2048;
const MAX_STRIPS: usize = 16_384;
const MAX_VERTICES: usize = 131_072;
const MAX_TEXTURES: usize = 32;
const MAX_KEYS: usize = 65_536;
const MAX_SOUND_CUES: usize = 64;
/// Register order of every GS A+D packet (CLAMP_1, MIPTBP1_1, MIPTBP2_1,
/// TEST_1, ALPHA_1, TEX0_1, TEX1_1).
const GS_REGISTERS: [u64; 7] = [0x08, 0x34, 0x36, 0x47, 0x42, 0x06, 0x14];
const SECTION_NAMES: [&str; 10] = [
    "nodes",
    "dma",
    "geometry",
    "dma2",
    "geometry2",
    "texture_table",
    "relocation1",
    "relocation2",
    "relocation_load",
    "texture_variants",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterModel {
    pub schema_version: u32,
    pub profile_id: String,
    /// Card id = slot index.
    pub card: u16,
    pub archive_offset: u64,
    pub slot_sha256: String,
    /// RH0 header word 1 (record chain flags).
    pub flags: u32,
    pub rh0_bytes: u32,
    /// RH0 header word 2 (end of the section area = first texture).
    pub header_end: u32,
    /// The complete 0xB0-byte RH0 header as little-endian words.
    pub raw_header_words: Vec<u32>,
    pub sections: Vec<MonsterSection>,
    /// f32 at RH0 +0x64: translation keys of nodes other than node 0 are
    /// multiplied by it; node 1's translation divided by it is the centre.
    pub inv_root_scale: f32,
    /// Runtime matrix blocks reserved at the head of DMA 1.
    pub matrix_blocks: u16,
    pub nodes: Vec<MonsterNode>,
    pub textures: Vec<MonsterTexture>,
    /// Pass 0 = DMA 1 (opaque), pass 1 = DMA 2 (blended), when present.
    pub passes: Vec<MonsterPass>,
    pub motions: Vec<MonsterMotion>,
    pub effect_textures: Vec<MonsterEffectTexture>,
    pub camera_records: Vec<MonsterRawRecord>,
    pub sound_cues: Vec<MonsterSoundCue>,
    /// Slot bytes used by the RH0 and the chained records.
    pub used_slot_bytes: u32,
    pub unresolved: Vec<String>,
    pub rendering_validated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterSection {
    pub name: String,
    pub offset: u32,
    pub byte_len: u32,
}

/// One 64-byte node (0x2100D0): +0 scale, +0xC flags | matrix block << 16,
/// +0x10 rotation (0x10000 = 360 degrees), +0x1C child byte offset, +0x20
/// translation (parent accumulated-scale units), +0x2C sibling byte offset,
/// +0x30 bounding sphere.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterNode {
    pub scale: [f32; 3],
    /// Bit 0 no translation, bit 1 no rotation, bit 2 no scale, bit 15 no
    /// geometry; bits 3-4 are set in some models (meaning unresolved).
    pub flags: u16,
    /// DMA-1 qword index of this node's runtime matrix block (0 = none).
    pub matrix_block: u16,
    pub rotation: [i32; 3],
    pub child: Option<u16>,
    pub sibling: Option<u16>,
    pub translation: [f32; 3],
    pub bounding_sphere: [f32; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterTexture {
    pub index: u16,
    /// Original 16-byte table entry: word 0 RH2 offset, word 2 an unresolved
    /// variant pointer (non-zero for some textures), words 1/3 zero.
    pub entry_words: [u32; 4],
    /// Tagged TEX0 patch locations (tag 2 = DMA 1, tag 4 = DMA 2).
    pub tex0_patches_raw: Vec<u32>,
    pub record: Option<MonsterTextureRecord>,
    /// Why no record is available (entry without an RH2, or a layout the RH2
    /// decoder does not support).
    pub unsupported: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterTextureRecord {
    pub slot_offset: u32,
    pub byte_len: u32,
    pub width: u32,
    pub height: u32,
    /// 16 (C_16, alpha bit) or 32 (C_32).
    pub palette_bits: u8,
    /// All 0x60 RH2 header bytes; words 12/13 hold the stored TEX0.
    pub raw_header_words: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterPass {
    pub pass: u8,
    pub draws: Vec<MonsterDraw>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterDraw {
    /// Slot offset of the geometry REF tag.
    pub dma_tag_offset: u32,
    /// Slot byte range of the geometry batch.
    pub geometry_start: u32,
    pub geometry_end: u32,
    /// Texture selected by the TEX0 patch of the active GS packet.
    pub texture_index: Option<u16>,
    /// Active material (ambient, diffuse + alpha, maximum) as raw f32 bits;
    /// None when the pass has not uploaded one yet.
    pub material_words: Option<[u32; 12]>,
    /// Active GS (value, register) pairs; None when this pass has not sent a
    /// GS packet before the draw (state from elsewhere, unresolved).
    pub gs_registers: Option<Vec<[u64; 2]>>,
    /// Cumulative VU slot -> node matrix block uploads of this pass.
    pub matrix_slots: Vec<MonsterMatrixSlot>,
    pub strips: Vec<MonsterStrip>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterMatrixSlot {
    pub vu_address: u16,
    pub matrix_block: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterStrip {
    pub record_offset: u32,
    pub skinned: bool,
    /// GS PRIM of the GIF tag (triangle strip; IIP, TME, FGE, ABE bits).
    pub prim: u16,
    pub gif_tag_words: Vec<u32>,
    /// VU matrix slot of a rigid strip (GIF tag word 3).
    pub rigid_matrix_vu: Option<u16>,
    pub vertices: Vec<MonsterVertex>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterVertex {
    /// (u, v, third lane): S = u / 4096, T = v / 4096; the third lane is the
    /// influence count of a weighted vertex and an unresolved 0/1 flag of a
    /// rigid one.
    pub texture_raw: [i16; 3],
    pub influences: Vec<MonsterInfluence>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterInfluence {
    pub matrix_vu: u16,
    /// Weight times 4096; a rigid vertex has one implicit influence of 4096.
    pub weight_raw: i16,
    /// Third lane of a weighted influence (0 or 1, meaning unresolved).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_raw: Option<i16>,
    /// Normal = raw / 32768 in bone space.
    pub normal_raw: [i16; 3],
    pub position_raw: [i16; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterMotion {
    /// Motion slot 0..=7 (header flag bit).
    pub motion: u8,
    pub slot_offset: u32,
    pub byte_len: u32,
    pub raw_header_words: [u32; 8],
    pub node_count: u16,
    /// Length in frames (obj+0x1C0 + 4 * motion).
    pub length: u32,
    /// 3 = translation + rotation, 7 = + scale.
    pub kind: u16,
    pub channels_raw: u16,
    pub header_word3_raw: u32,
    /// Per-node u16 map (mostly 0xFFFF; unused by the evaluator, unresolved).
    pub node_map_raw: Vec<u16>,
    pub tracks: Vec<MonsterTrack>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MonsterTrack {
    pub translation: Option<Vec<MonsterVecKey>>,
    pub rotation: Option<Vec<MonsterAngleKey>>,
    pub scale: Option<Vec<MonsterVecKey>>,
}

/// (frame, value).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterVecKey(pub u32, pub [f32; 3]);
/// (frame, angles with 0x10000 = 360 degrees).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterAngleKey(pub u32, pub [i32; 3]);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonsterEffectTexture {
    /// Effect slot 0..=7 (obj+0x144 + 4 * slot, used by the attack 0x1F1EE0).
    pub effect_slot: u8,
    pub slot_offset: u32,
    pub byte_len: u32,
    pub record: Option<MonsterTextureRecord>,
    pub unsupported: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterRawRecord {
    /// "cmot_0x16c" or "cmot_0x170" (object field the loader stores it in).
    pub kind: String,
    pub slot_offset: u32,
    pub byte_len: u32,
    pub sha256: String,
    pub raw_header_words: [u32; 4],
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterSoundCue {
    pub motion: u8,
    /// Fires when the motion time passes this frame (t_prev, t].
    pub frame: u16,
    pub sound: u16,
}

/// Which texture of a model to decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonsterTextureRef {
    Model(usize),
    Effect(usize),
}

impl MonsterDraw {
    pub fn material(&self) -> Option<[f32; 12]> {
        self.material_words.map(|w| w.map(f32::from_bits))
    }
    pub fn gs_register(&self, address: u64) -> Option<u64> {
        self.gs_registers
            .as_ref()?
            .iter()
            .find(|pair| pair[1] == address)
            .map(|pair| pair[0])
    }
    pub fn matrix_block(&self, vu_address: u16) -> Option<u16> {
        self.matrix_slots
            .iter()
            .find(|slot| slot.vu_address == vu_address)
            .map(|slot| slot.matrix_block)
    }
}

impl MonsterModel {
    pub fn motion(&self, motion: u8) -> Option<&MonsterMotion> {
        self.motions.iter().find(|m| m.motion == motion)
    }
    /// Table index of the node owning a matrix block.
    pub fn node_of_block(&self, block: u16) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.matrix_block == block && block != 0)
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(format!("PAL monster model: {}", message.into()))
}
fn unsupported(message: impl Into<String>) -> Error {
    Error::Unsupported(format!("PAL monster model: {}", message.into()))
}
fn half(bytes: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        region(bytes, at, 2)?.try_into().unwrap(),
    ))
}
fn short(bytes: &[u8], at: usize) -> Result<i16> {
    Ok(i16::from_le_bytes(
        region(bytes, at, 2)?.try_into().unwrap(),
    ))
}
fn float(bytes: &[u8], at: usize) -> Result<f32> {
    let value = f32::from_bits(word(bytes, at)?);
    if !value.is_finite() {
        return Err(invalid(format!("non-finite float at {at:#x}")));
    }
    Ok(value)
}
fn floats<const N: usize>(bytes: &[u8], at: usize) -> Result<[f32; N]> {
    let mut out = [0.0; N];
    for (i, value) in out.iter_mut().enumerate() {
        *value = float(bytes, at + 4 * i)?;
    }
    Ok(out)
}
fn words_at<const N: usize>(bytes: &[u8], at: usize) -> Result<[u32; N]> {
    let mut out = [0; N];
    for (i, value) in out.iter_mut().enumerate() {
        *value = word(bytes, at + 4 * i)?;
    }
    Ok(out)
}

/// Read the 1025-entry quarter sine table of 0x20ACC0 from the fingerprinted
/// executable. Values are validated as a monotonic table from 0 to 1.
pub fn parse_pal_sine_table(executable: &[u8]) -> Result<Vec<f32>> {
    require_pal_executable(executable)?;
    let table: Vec<f32> = (0..PAL_SINE_TABLE_ENTRIES)
        .map(|i| float(executable, PAL_SINE_TABLE_FILE_OFFSET + 4 * i))
        .collect::<Result<_>>()?;
    if table[0] != 0.0
        || table[PAL_SINE_TABLE_ENTRIES - 1] != 1.0
        || table.windows(2).any(|pair| pair[1] < pair[0])
    {
        return Err(invalid("sine table is not a monotonic quarter wave"));
    }
    Ok(table)
}

/// Decode one complete 1 MiB MONSTER.MRG slot (card id = slot index).
pub fn decode_pal_monster_slot(slot: &[u8], card: u16) -> Result<MonsterModel> {
    if slot.len() != PAL_MONSTER_SLOT_BYTES {
        return Err(invalid("slot must be exactly 1 MiB"));
    }
    if usize::from(card) >= PAL_MONSTER_SLOT_COUNT {
        return Err(invalid(format!("card {card} has no MONSTER.MRG slot")));
    }
    if slot.get(..4) != Some(b"RH0\0") {
        return Err(invalid("slot does not start with an RH0 model"));
    }
    let flags = word(slot, 4)?;
    let header_end = word(slot, 8)? as usize;
    let rh0_bytes = word(slot, 12)? as usize;
    if flags & 0x8000_0000 == 0 || flags & 0x7FC0_0000 != 0 {
        return Err(unsupported(format!("RH0 flags {flags:#010x}")));
    }
    if flags & 0x700 != 0 {
        return Err(unsupported(
            "0x8000-byte blocks or a chained RH2 (flag bits 8-10) are not supported",
        ));
    }
    if !(RH0_HEADER_BYTES..=slot.len()).contains(&rh0_bytes)
        || !rh0_bytes.is_multiple_of(16)
        || !(RH0_HEADER_BYTES..=rh0_bytes).contains(&header_end)
    {
        return Err(invalid("RH0 size or header end out of bounds"));
    }
    let rh0 = &slot[..rh0_bytes];
    let mut sections = Vec::new();
    let mut expected = RH0_HEADER_BYTES;
    for (index, name) in SECTION_NAMES.iter().enumerate() {
        let offset = word(rh0, 0x10 + 8 * index)? as usize;
        let byte_len = word(rh0, 0x14 + 8 * index)? as usize;
        if offset != expected || !byte_len.is_multiple_of(16) {
            return Err(invalid("RH0 sections are not contiguous and aligned"));
        }
        region(rh0, offset, byte_len)?;
        expected = offset + byte_len;
        sections.push(MonsterSection {
            name: (*name).into(),
            offset: offset as u32,
            byte_len: byte_len as u32,
        });
    }
    if expected != header_end {
        return Err(invalid("RH0 sections do not end at the header end"));
    }
    let span = |index: usize| {
        let s = &sections[index];
        (s.offset as usize, (s.offset + s.byte_len) as usize)
    };
    let inv_root_scale = float(rh0, 0x64)?;
    if inv_root_scale <= 0.0 {
        return Err(invalid("inverse root scale must be positive"));
    }
    let nodes = parse_nodes(rh0, span(0))?;
    let (dma1_start, dma1_end) = span(1);
    let matrix_blocks = matrix_block_count(rh0, dma1_start, dma1_end)?;
    for node in &nodes {
        let block = usize::from(node.matrix_block);
        if block != 0 && !(block % 7 == 1 && (block - 1) / 7 < usize::from(matrix_blocks)) {
            return Err(invalid("node matrix block outside the reserved area"));
        }
    }
    let (textures, patches) = parse_texture_table(rh0, span(5), span(1), span(3), header_end)?;
    let mut passes = Vec::new();
    let mut used_patches = BTreeSet::new();
    for (pass, dma, geometry) in [(0u8, span(1), span(2)), (1u8, span(3), span(4))] {
        // DMA 1 always holds the matrix blocks; some models draw only in the
        // blended pass (their DMA 1 ends right after the blocks).
        let chain_start = if pass == 0 {
            dma.0 + 0x10 + usize::from(matrix_blocks) * MATRIX_BLOCK_BYTES
        } else {
            dma.0
        };
        if chain_start == dma.1 {
            if geometry.0 != geometry.1 {
                return Err(invalid("geometry section without a DMA chain"));
            }
            continue;
        }
        let draws = parse_pass(
            rh0,
            pass,
            dma,
            geometry,
            matrix_blocks,
            &nodes,
            &patches,
            &mut used_patches,
        )?;
        passes.push(MonsterPass { pass, draws });
    }
    if passes.is_empty() {
        return Err(invalid("model has no draw pass"));
    }
    if used_patches.len() != patches.len() {
        return Err(invalid("TEX0 patch outside every GS packet of the draws"));
    }
    let strips: usize = passes
        .iter()
        .flat_map(|p| &p.draws)
        .map(|d| d.strips.len())
        .sum();
    let vertices: usize = passes
        .iter()
        .flat_map(|p| &p.draws)
        .flat_map(|d| &d.strips)
        .map(|s| s.vertices.len())
        .sum();
    if strips > MAX_STRIPS || vertices > MAX_VERTICES {
        return Err(invalid("geometry exceeds supported bounds"));
    }

    // Records chained after the RH0 (loader 0x1F1710).
    let mut at = rh0_bytes;
    let mut motions = Vec::new();
    for motion in 0..8u8 {
        if flags & (1 << motion) == 0 {
            continue;
        }
        let parsed = parse_motion(slot, at, motion, nodes.len())?;
        at += parsed.byte_len as usize;
        motions.push(parsed);
    }
    let mut effect_textures = Vec::new();
    for effect_slot in 0..8u8 {
        if flags & (1 << (11 + effect_slot)) == 0 {
            continue;
        }
        if region(slot, at, 4)? != b"RH2\0" {
            return Err(invalid("effect texture record is not RH2"));
        }
        let byte_len = word(slot, at + 8)? as usize;
        let bytes = region(slot, at, byte_len)?;
        let (record, unsupported) = texture_record(bytes, at);
        effect_textures.push(MonsterEffectTexture {
            effect_slot,
            slot_offset: at as u32,
            byte_len: byte_len as u32,
            record,
            unsupported,
        });
        at += byte_len;
    }
    let mut camera_records = Vec::new();
    for (bit, kind) in [(19, "cmot_0x16c"), (20, "cmot_0x170")] {
        if flags & (1 << bit) == 0 {
            continue;
        }
        let stored = word(slot, at + 8)? as usize;
        let byte_len = stored
            .checked_add(15)
            .map(|n| n & !15)
            .filter(|n| *n >= 16)
            .ok_or_else(|| invalid("camera record length"))?;
        let bytes = region(slot, at, byte_len)?;
        camera_records.push(MonsterRawRecord {
            kind: kind.into(),
            slot_offset: at as u32,
            byte_len: byte_len as u32,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            raw_header_words: words_at(slot, at)?,
        });
        at += byte_len;
    }
    let mut sound_cues = Vec::new();
    if flags & (1 << 21) != 0 {
        let mut terminated = false;
        for i in 0..MAX_SOUND_CUES {
            let key = half(slot, at + 4 * i)?;
            if key & 15 == 15 {
                terminated = true;
                at += 4 * (i + 1);
                break;
            }
            sound_cues.push(MonsterSoundCue {
                motion: (key & 15) as u8,
                frame: key >> 4,
                sound: half(slot, at + 4 * i + 2)?,
            });
        }
        if !terminated {
            return Err(invalid(
                "sound-cue table has no terminator within 64 entries",
            ));
        }
    }
    if at > slot.len() {
        return Err(invalid("chained records exceed the slot"));
    }
    let mut unresolved = vec![
        "MOT per-node u16 map (node_map_raw)".to_string(),
        "texture-table variant words and RH0 section 9 texture variants (0x211700)".into(),
        "rigid vertex UV third lane (0/1)".into(),
        "third lane of weighted influences (0/1)".into(),
    ];
    if !camera_records.is_empty() {
        unresolved.push("CMOT camera records (kept raw)".into());
    }
    if textures.iter().any(|t| t.record.is_none())
        || effect_textures.iter().any(|t| t.record.is_none())
    {
        unresolved.push("textures without a supported RH2 record".into());
    }
    if passes
        .iter()
        .flat_map(|p| &p.draws)
        .any(|d| d.gs_registers.is_none())
    {
        unresolved.push("draws whose pass sends no GS packet before them".into());
    }
    Ok(MonsterModel {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        card,
        archive_offset: u64::from(card) * PAL_MONSTER_SLOT_BYTES as u64,
        slot_sha256: format!("{:x}", Sha256::digest(slot)),
        flags,
        rh0_bytes: rh0_bytes as u32,
        header_end: header_end as u32,
        raw_header_words: words_at::<44>(rh0, 0)?.to_vec(),
        sections,
        inv_root_scale,
        matrix_blocks,
        nodes,
        textures,
        passes,
        motions,
        effect_textures,
        camera_records,
        sound_cues,
        used_slot_bytes: at as u32,
        unresolved,
        rendering_validated: false,
    })
}

/// Decode one model or effect texture of a decoded slot into lossless
/// indexed form. The slot must be the one the model was decoded from.
/// GS texel bytes (RGBA) of a decoded model texture: C_16 RGB5 << 3 with
/// alpha 128 when the CLUT alpha bit is set (TEXA TA1) and 0 otherwise
/// (TA0); C_32 raw bytes (alpha 128 = 1.0).
pub fn monster_gs_texels(texture: &OpeningIndexedTexture) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(texture.palette_indices.len() * 4);
    for index in &texture.palette_indices {
        let value = texture.palette_words_raw[usize::from(*index)];
        pixels.extend(match texture.palette_format {
            OpeningPaletteFormat::Rgb5551 => [
                ((value & 31) << 3) as u8,
                (((value >> 5) & 31) << 3) as u8,
                (((value >> 10) & 31) << 3) as u8,
                if value & 0x8000 != 0 { 128 } else { 0 },
            ],
            OpeningPaletteFormat::Rgba8888Alpha128 => value.to_le_bytes(),
        });
    }
    pixels
}

pub fn decode_pal_monster_texture(
    slot: &[u8],
    model: &MonsterModel,
    texture: MonsterTextureRef,
) -> Result<OpeningIndexedTexture> {
    if slot.len() != PAL_MONSTER_SLOT_BYTES
        || format!("{:x}", Sha256::digest(slot)) != model.slot_sha256
    {
        return Err(invalid("slot does not match the decoded model"));
    }
    let (record, reason) = match texture {
        MonsterTextureRef::Model(i) => model
            .textures
            .get(i)
            .map(|t| (t.record.as_ref(), t.unsupported.as_deref())),
        MonsterTextureRef::Effect(i) => model
            .effect_textures
            .get(i)
            .map(|t| (t.record.as_ref(), t.unsupported.as_deref())),
    }
    .ok_or_else(|| invalid("texture index out of range"))?;
    let record = record.ok_or_else(|| {
        unsupported(format!(
            "texture has no decodable record: {}",
            reason.unwrap_or("unknown")
        ))
    })?;
    let start = record.slot_offset as usize;
    let bytes = region(slot, start, record.byte_len as usize)?;
    let inspected = opening_textures::inspect_record(bytes, 0, start)?;
    if inspected.width != record.width || inspected.height != record.height {
        return Err(invalid("texture record does not match the slot"));
    }
    opening_textures::decode_indexed_record(slot, &inspected)
}

fn texture_record(
    bytes: &[u8],
    slot_offset: usize,
) -> (Option<MonsterTextureRecord>, Option<String>) {
    match opening_textures::inspect_record(bytes, 0, slot_offset) {
        Ok(record) => (
            Some(MonsterTextureRecord {
                slot_offset: slot_offset as u32,
                byte_len: record.byte_len,
                width: record.width,
                height: record.height,
                palette_bits: match record.palette_format {
                    OpeningPaletteFormat::Rgb5551 => 16,
                    OpeningPaletteFormat::Rgba8888Alpha128 => 32,
                },
                raw_header_words: record.raw_header_words.to_vec(),
            }),
            None,
        ),
        Err(error) => {
            let (width, height) = (
                word(bytes, 0x54).map(|w| w & 0xffff).unwrap_or(0),
                word(bytes, 0x54).map(|w| w >> 16).unwrap_or(0),
            );
            (
                None,
                Some(format!(
                    "RH2 {width}x{height} at slot {slot_offset:#x}: {error}"
                )),
            )
        }
    }
}

fn parse_nodes(rh0: &[u8], (start, end): (usize, usize)) -> Result<Vec<MonsterNode>> {
    let bytes = end - start;
    if bytes == 0 || !bytes.is_multiple_of(NODE_BYTES) || bytes / NODE_BYTES > MAX_NODES {
        return Err(invalid("unsupported node count"));
    }
    let count = bytes / NODE_BYTES;
    let link = |value: u32| -> Result<Option<u16>> {
        let value = value as usize;
        if value == 0 {
            return Ok(None);
        }
        if !value.is_multiple_of(NODE_BYTES) || value / NODE_BYTES >= count {
            return Err(invalid("node link outside the node table"));
        }
        Ok(Some((value / NODE_BYTES) as u16))
    };
    let mut nodes = Vec::with_capacity(count);
    for index in 0..count {
        let at = start + index * NODE_BYTES;
        let flag_word = word(rh0, at + 0xC)?;
        nodes.push(MonsterNode {
            scale: floats(rh0, at)?,
            flags: flag_word as u16,
            matrix_block: (flag_word >> 16) as u16,
            rotation: words_at::<3>(rh0, at + 0x10)?.map(|w| w as i32),
            child: link(word(rh0, at + 0x1C)?)?,
            translation: floats(rh0, at + 0x20)?,
            sibling: link(word(rh0, at + 0x2C)?)?,
            bounding_sphere: floats(rh0, at + 0x30)?,
        });
    }
    // Walk from node 0 like 0x2100D0: every node exactly once, in table order.
    let mut order = Vec::with_capacity(count);
    let mut stack = vec![Some(0u16)];
    while let Some(entry) = stack.pop() {
        let mut current = entry;
        while let Some(index) = current {
            if order.len() >= count || order.contains(&index) {
                return Err(invalid("node links revisit a node"));
            }
            order.push(index);
            let node = &nodes[usize::from(index)];
            if node.child.is_some() {
                stack.push(node.sibling);
                current = node.child;
            } else {
                current = node.sibling;
            }
        }
    }
    if order.len() != count || order.iter().enumerate().any(|(i, n)| usize::from(*n) != i) {
        return Err(invalid("node table is not the preorder of its links"));
    }
    Ok(nodes)
}

/// The opaque chain starts with NEXT over the zeroed runtime matrix blocks.
fn matrix_block_count(rh0: &[u8], start: usize, end: usize) -> Result<u16> {
    let [tag, address, vif0, vif1] = words_at::<4>(rh0, start)?;
    let jump = (address & 0x0fff_ffff) as usize;
    if tag != 0x2000_0000
        || address >> 28 != 2
        || vif0 != 0x0300_0080
        || vif1 != 0x0200_01c0
        || jump < 0x10
        || !(jump - 0x10).is_multiple_of(MATRIX_BLOCK_BYTES)
        || start + jump > end
    {
        return Err(invalid("unsupported initial DMA jump"));
    }
    if region(rh0, start + 16, jump - 16)?.iter().any(|b| *b != 0) {
        return Err(invalid("runtime matrix blocks are not zero on disc"));
    }
    Ok(((jump - 0x10) / MATRIX_BLOCK_BYTES) as u16)
}

type PatchMap = BTreeMap<(u8, usize), u16>;

fn parse_texture_table(
    rh0: &[u8],
    (start, end): (usize, usize),
    dma1: (usize, usize),
    dma2: (usize, usize),
    header_end: usize,
) -> Result<(Vec<MonsterTexture>, PatchMap)> {
    let count = word(rh0, start)? as usize;
    let refs = word(rh0, start + 4)? as usize;
    if count > MAX_TEXTURES || refs != 16 + 16 * count || start + refs + 8 * count > end {
        return Err(invalid("invalid texture table"));
    }
    let mut textures = Vec::with_capacity(count);
    let mut patches = PatchMap::new();
    let mut previous_end = header_end;
    for index in 0..count {
        let entry_words = words_at::<4>(rh0, start + 16 + 16 * index)?;
        if entry_words[1] != 0 || entry_words[3] != 0 {
            return Err(unsupported("texture table entry words 1/3"));
        }
        let variant = entry_words[2] as usize;
        if variant != 0 && !(header_end..rh0.len()).contains(&variant) {
            return Err(invalid("texture variant pointer outside the texture area"));
        }
        let n = word(rh0, start + refs + 8 * index)? as usize;
        let list = word(rh0, start + refs + 8 * index + 4)? as usize;
        if n > 64 || (n > 0 && (list < refs + 8 * count || start + list + 4 * n > end)) {
            return Err(invalid("texture patch list exceeds the table"));
        }
        let mut raw = Vec::with_capacity(n);
        for j in 0..n {
            let tagged = word(rh0, start + list + 4 * j)?;
            let (pass, section) = match tagged >> 28 {
                2 => (0u8, dma1),
                4 => (1u8, dma2),
                _ => return Err(invalid("TEX0 patch tag")),
            };
            let at = section.0 + (tagged & 0x0fff_ffff) as usize;
            if at + 16 > section.1
                || wide(rh0, at + 8)? != 6
                || wide(rh0, at)? != 0
                || patches.insert((pass, at), index as u16).is_some()
            {
                return Err(invalid("invalid or duplicate TEX0 patch"));
            }
            raw.push(tagged);
        }
        let offset = entry_words[0] as usize;
        let (record, unsupported) = if offset == 0 {
            (
                None,
                Some("texture table entry without an RH2 record".to_string()),
            )
        } else {
            if offset < previous_end
                || !offset.is_multiple_of(16)
                || region(rh0, offset, 4)? != b"RH2\0"
            {
                return Err(invalid("embedded texture offset"));
            }
            let len = word(rh0, offset + 8)? as usize;
            let bytes = region(rh0, offset, len)?;
            previous_end = offset + len;
            texture_record(bytes, offset)
        };
        textures.push(MonsterTexture {
            index: index as u16,
            entry_words,
            tex0_patches_raw: raw,
            record,
            unsupported,
        });
    }
    Ok((textures, patches))
}

#[allow(clippy::too_many_arguments)]
fn parse_pass(
    rh0: &[u8],
    pass: u8,
    (dma_start, dma_end): (usize, usize),
    (geo_start, geo_end): (usize, usize),
    matrix_blocks: u16,
    nodes: &[MonsterNode],
    patches: &PatchMap,
    used_patches: &mut BTreeSet<(u8, usize)>,
) -> Result<Vec<MonsterDraw>> {
    let (matrix_tag, geometry_tag) = if pass == 0 { (2, 3) } else { (4, 5) };
    let mut at = dma_start;
    if pass == 0 {
        at += 0x10 + usize::from(matrix_blocks) * MATRIX_BLOCK_BYTES;
    }
    let node_blocks: BTreeSet<u16> = nodes
        .iter()
        .map(|n| n.matrix_block)
        .filter(|b| *b != 0)
        .collect();
    let mut texture = None;
    let mut material = None;
    let mut registers: Option<Vec<[u64; 2]>> = None;
    let mut slots: BTreeMap<u16, u16> = BTreeMap::new();
    let mut draws = Vec::new();
    let mut next_geometry = geo_start;
    while at < dma_end {
        if at + 16 > dma_end {
            return Err(invalid("truncated DMA tag"));
        }
        let [tag, address, vif0, vif1] = words_at::<4>(rh0, at)?;
        let qwords = (tag & 0xffff) as usize;
        // Only the ID field may be set besides QWC (no IRQ/PCE bits).
        if tag & 0x8fff_0000 != 0 {
            return Err(unsupported("DMA tag flags"));
        }
        match (tag >> 28) & 7 {
            1 => {
                let payload = at + 16;
                if address != 0 || payload + 16 * qwords > dma_end {
                    return Err(invalid("DMA payload exceeds the chain"));
                }
                match (qwords, vif0, vif1) {
                    (8, 0x1100_0000, 0x5100_0008) => {
                        if wide(rh0, payload)? != 0x1000_0000_0000_8007
                            || wide(rh0, payload + 8)? != 14
                        {
                            return Err(unsupported("GS register packet GIF tag"));
                        }
                        let pairs: Vec<[u64; 2]> = (0..7)
                            .map(|i| {
                                Ok([
                                    wide(rh0, payload + 16 + 16 * i)?,
                                    wide(rh0, payload + 24 + 16 * i)?,
                                ])
                            })
                            .collect::<Result<_>>()?;
                        if pairs.iter().map(|p| p[1]).ne(GS_REGISTERS) {
                            return Err(unsupported("GS register order"));
                        }
                        let tex0 = payload + 16 + 5 * 16;
                        texture = patches.get(&(pass, tex0)).copied();
                        if texture.is_some() {
                            used_patches.insert((pass, tex0));
                        }
                        registers = Some(pairs);
                    }
                    (3, 0x1000_0000, 0x6c03_003c) => {
                        let values = words_at::<12>(rh0, payload)?;
                        if values.iter().any(|w| !f32::from_bits(*w).is_finite()) {
                            return Err(invalid("non-finite material"));
                        }
                        material = Some(values);
                    }
                    _ => return Err(unsupported("DMA inline packet")),
                }
                at = payload + 16 * qwords;
            }
            3 => {
                let relative = (address & 0x0fff_ffff) as usize;
                let base = address >> 28;
                if qwords == 7 && vif0 == 0x1000_0000 && vif1 >> 16 == 0x6c07 {
                    let vu = (vif1 & 0xffff) as u16;
                    let block = relative / 16;
                    if base != matrix_tag
                        || vu < 4
                        || (vu - 4) % 7 != 0
                        || !relative.is_multiple_of(16)
                        || block > usize::from(u16::MAX)
                        || !node_blocks.contains(&(block as u16))
                    {
                        return Err(invalid("matrix upload is not a node block"));
                    }
                    slots.insert(vu, block as u16);
                } else if qwords > 0 && vif0 == 0 && vif1 == 0 && base == geometry_tag {
                    let start = geo_start + relative;
                    let end = start + 16 * qwords;
                    if start != next_geometry || end > geo_end || draws.len() >= MAX_DRAWS {
                        return Err(invalid("geometry references overlap or leave the section"));
                    }
                    next_geometry = end;
                    let matrix_slots: Vec<MonsterMatrixSlot> = slots
                        .iter()
                        .map(|(vu, block)| MonsterMatrixSlot {
                            vu_address: *vu,
                            matrix_block: *block,
                        })
                        .collect();
                    let strips = parse_strips(rh0, start, end, &slots)?;
                    draws.push(MonsterDraw {
                        dma_tag_offset: at as u32,
                        geometry_start: start as u32,
                        geometry_end: end as u32,
                        texture_index: texture,
                        material_words: material,
                        gs_registers: registers.clone(),
                        matrix_slots,
                        strips,
                    });
                } else {
                    return Err(unsupported("DMA reference"));
                }
                at += 16;
            }
            _ => return Err(unsupported("DMA tag kind")),
        }
    }
    if at != dma_end || next_geometry != geo_end || draws.is_empty() {
        return Err(invalid("draws do not cover the DMA chain and geometry"));
    }
    Ok(draws)
}

/// One geometry batch: GIF tag UNPACK (V4-32 rigid, V3-32 weighted), then a
/// V3-16 vertex UNPACK; MSCALF 0 and NOP between batches.
fn parse_strips(
    rh0: &[u8],
    start: usize,
    end: usize,
    slots: &BTreeMap<u16, u16>,
) -> Result<Vec<MonsterStrip>> {
    let mut at = start;
    let mut strips = Vec::new();
    while at < end {
        let command = word(rh0, at)?;
        if command == 0 || command == 0x1500_0000 {
            at += 4;
            continue;
        }
        let (skinned, tag_words) = match command >> 16 {
            0x6c01 => (false, 4usize),
            0x6801 => (true, 3usize),
            _ => return Err(unsupported(format!("geometry VIF command {command:#010x}"))),
        };
        if command & 0xfc00 != 0x8000 || at + 4 + 4 * tag_words + 4 > end {
            return Err(unsupported("geometry GIF tag unpack"));
        }
        let gif: Vec<u32> = (0..tag_words)
            .map(|i| word(rh0, at + 4 + 4 * i))
            .collect::<Result<_>>()?;
        let nloop = (gif[0] & 0x7fff) as usize;
        let prim = ((gif[1] >> 15) & 0x7ff) as u16;
        let expected_regs = if skinned { 0x8412 } else { 0x412 };
        if gif[0] & !0xffff != 0
            || gif[1] & 0x3fff != 0
            || gif[1] >> 26 != 0x0C
            || (gif[1] >> 14) & 1 != 1
            || prim & 7 != 4
            || prim & !0x7f != 0
            || gif[2] != expected_regs
            || nloop < 3
        {
            return Err(unsupported(format!(
                "strip GIF tag {:#010x} {:#010x} {:#x}",
                gif[0], gif[1], gif[2]
            )));
        }
        let rigid_matrix_vu = if skinned {
            None
        } else {
            let vu = gif[3];
            if vu > 0x3ff || !slots.contains_key(&(vu as u16)) {
                return Err(invalid("rigid strip matrix slot was never uploaded"));
            }
            Some(vu as u16)
        };
        let data_at = at + 4 + 4 * tag_words;
        let data = word(rh0, data_at)?;
        let num = match (data >> 16) & 0xff {
            0 => 256,
            n => n as usize,
        };
        if data >> 24 != 0x69 || data & 0xfc00 != 0x8000 {
            return Err(unsupported("vertex UNPACK command"));
        }
        let values_at = data_at + 4;
        let padded = (6 * num + 3) & !3;
        if values_at + padded > end {
            return Err(invalid("vertex data exceeds the batch"));
        }
        let lane = |i: usize, k: usize| short(rh0, values_at + 6 * i + 2 * k);
        let triple = |i: usize| -> Result<[i16; 3]> { Ok([lane(i, 0)?, lane(i, 1)?, lane(i, 2)?]) };
        let mut vertices = Vec::with_capacity(nloop);
        let mut i = 0usize;
        for _ in 0..nloop {
            if i >= num {
                return Err(invalid("vertex data shorter than NLOOP"));
            }
            let texture_raw = triple(i)?;
            i += 1;
            let mut influences = Vec::new();
            if skinned {
                let k = texture_raw[2];
                if !(1..=8).contains(&k) || i + 3 * k as usize > num {
                    return Err(invalid("weighted vertex influence count"));
                }
                for _ in 0..k {
                    let [vu, weight, extra] = triple(i)?;
                    if vu < 0 || !slots.contains_key(&(vu as u16)) {
                        return Err(invalid("weighted influence matrix slot was never uploaded"));
                    }
                    influences.push(MonsterInfluence {
                        matrix_vu: vu as u16,
                        weight_raw: weight,
                        lane_raw: Some(extra),
                        normal_raw: triple(i + 1)?,
                        position_raw: triple(i + 2)?,
                    });
                    i += 3;
                }
            } else {
                if i + 2 > num {
                    return Err(invalid("rigid vertex truncated"));
                }
                influences.push(MonsterInfluence {
                    matrix_vu: rigid_matrix_vu.unwrap(),
                    weight_raw: 4096,
                    lane_raw: None,
                    normal_raw: triple(i)?,
                    position_raw: triple(i + 1)?,
                });
                i += 2;
            }
            vertices.push(MonsterVertex {
                texture_raw,
                influences,
            });
        }
        if i != num {
            return Err(invalid("vertex data length disagrees with NLOOP"));
        }
        if region(rh0, values_at + 6 * num, padded - 6 * num)?
            .iter()
            .any(|b| *b != 0)
        {
            return Err(invalid("nonzero VIF padding"));
        }
        strips.push(MonsterStrip {
            record_offset: at as u32,
            skinned,
            prim,
            gif_tag_words: gif,
            rigid_matrix_vu,
            vertices,
        });
        at = values_at + padded;
    }
    if at != end {
        return Err(invalid("geometry batch overruns its range"));
    }
    Ok(strips)
}

fn parse_motion(slot: &[u8], base: usize, motion: u8, model_nodes: usize) -> Result<MonsterMotion> {
    if region(slot, base, 4)? != b"MOT\x1a" {
        return Err(invalid("motion record is not MOT"));
    }
    let byte_len = word(slot, base + 4)? as usize;
    if byte_len < 0x20 {
        return Err(invalid("motion record length"));
    }
    let mot = region(slot, base, byte_len)?;
    let raw_header_words = words_at::<8>(mot, 0)?;
    let node_count = half(mot, 0x10)?;
    if mot[0x13] & 0x80 != 0 {
        return Err(unsupported(
            "MOT byte 0x13 bit 7 (never set in the supported archive)",
        ));
    }
    // Tracks past the model's node count (two slots) are never read by the
    // evaluator, which stops at the model's last preorder node.
    if node_count == 0 || usize::from(node_count) > MAX_NODES {
        return Err(invalid(format!(
            "motion node count {node_count} (model has {model_nodes})"
        )));
    }
    let header = word(mot, 0x14)? as usize;
    let map = word(mot, 0x18)? as usize;
    let node_map_raw = (0..usize::from(node_count))
        .map(|i| half(mot, map + 2 * i))
        .collect::<Result<Vec<_>>>()?;
    let tracks_at = word(mot, header)? as usize;
    let length = word(mot, header + 4)?;
    let kind = half(mot, header + 8)?;
    let channels_raw = half(mot, header + 10)?;
    let header_word3_raw = word(mot, header + 12)?;
    let channels = match (kind, channels_raw) {
        (3, 0x42) => 2usize,
        (7, 0x43) => 3usize,
        _ => {
            return Err(unsupported(format!(
                "motion type {kind:#x}/{channels_raw:#x}"
            )))
        }
    };
    if length == 0 {
        return Err(invalid("motion length is zero"));
    }
    let stride = 8 * channels;
    let mut tracks = Vec::with_capacity(usize::from(node_count));
    let mut total_keys = 0usize;
    for node in 0..usize::from(node_count) {
        let entry = tracks_at + stride * node;
        let mut track = MonsterTrack::default();
        for channel in 0..channels {
            let pointer = word(mot, entry + 4 * channel)? as usize;
            let count = word(mot, entry + 4 * (channels + channel))? as usize;
            if pointer == 0 {
                continue;
            }
            total_keys += count;
            if count == 0 || total_keys > MAX_KEYS {
                return Err(invalid("motion channel key count"));
            }
            region(mot, pointer, 16 * count)?;
            let mut frames = Vec::with_capacity(count);
            for k in 0..count {
                frames.push(word(mot, pointer + 16 * k)?);
            }
            if frames.windows(2).any(|pair| pair[1] < pair[0]) {
                return Err(invalid("motion key frames decrease"));
            }
            if channel == 1 {
                track.rotation = Some(
                    (0..count)
                        .map(|k| {
                            Ok(MonsterAngleKey(
                                frames[k],
                                words_at::<3>(mot, pointer + 16 * k + 4)?.map(|w| w as i32),
                            ))
                        })
                        .collect::<Result<_>>()?,
                );
            } else {
                let keys = (0..count)
                    .map(|k| Ok(MonsterVecKey(frames[k], floats(mot, pointer + 16 * k + 4)?)))
                    .collect::<Result<_>>()?;
                if channel == 0 {
                    track.translation = Some(keys);
                } else {
                    track.scale = Some(keys);
                }
            }
        }
        tracks.push(track);
    }
    Ok(MonsterMotion {
        motion,
        slot_offset: base as u32,
        byte_len: byte_len as u32,
        raw_header_words,
        node_count,
        length,
        kind,
        channels_raw,
        header_word3_raw,
        node_map_raw,
        tracks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn put_f(bytes: &mut [u8], at: usize, value: f32) {
        put(bytes, at, value.to_bits());
    }
    fn put_h(bytes: &mut [u8], at: usize, value: i16) {
        bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    const NODES: usize = 0xB0;
    const DMA: usize = NODES + 128;
    /// Jump (0x10) + one matrix block (0x70) + material (0x40) + two REFs.
    const DMA_LEN: usize = 0x80 + 0x60;
    const GEOMETRY: usize = DMA + DMA_LEN;
    const GEO_LEN: usize = 0xD0;
    const TEXTAB: usize = GEOMETRY + GEO_LEN;
    const HEADER_END: usize = TEXTAB + 64;
    const MOT: usize = HEADER_END;
    const MOT_LEN: usize = 0x100;
    const CUES: usize = MOT + MOT_LEN;
    /// First weighted strip's first influence (VU slot lane).
    const FIRST_INFLUENCE: usize = GEOMETRY + 80 + 4 + 20 + 6;

    /// A minimal slot: two nodes (root + one drawable node with block 1),
    /// one opaque pass with a rigid and a weighted strip, one idle motion
    /// and a sound-cue table. No textures (count 0).
    fn synthetic_slot() -> Vec<u8> {
        let mut s = vec![0u8; PAL_MONSTER_SLOT_BYTES];
        s[..4].copy_from_slice(b"RH0\0");
        let sections = [
            (NODES, 128),
            (DMA, DMA_LEN),
            (GEOMETRY, GEO_LEN),
            (TEXTAB, 0),
            (TEXTAB, 0),
            (TEXTAB, 16),
            (TEXTAB + 16, 16),
            (TEXTAB + 32, 0),
            (TEXTAB + 32, 16),
            (TEXTAB + 48, 16),
        ];
        for (i, (o, l)) in sections.iter().enumerate() {
            put(&mut s, 0x10 + 8 * i, *o as u32);
            put(&mut s, 0x14 + 8 * i, *l as u32);
        }
        put(&mut s, 8, HEADER_END as u32);
        put(&mut s, 12, HEADER_END as u32);
        put(&mut s, 4, 0x8020_0001);
        put_f(&mut s, 0x64, 100.0);
        // Nodes: root (child 1, no geometry), node 1 (block 1).
        for n in 0..2 {
            let at = NODES + 64 * n;
            for k in 0..3 {
                put_f(&mut s, at + 4 * k, if n == 0 { 0.01 } else { 1.0 });
            }
            put(&mut s, at + 0xC, if n == 0 { 0x8000 } else { 1 << 16 });
            put_f(&mut s, at + 0x3C, 20.0);
        }
        put(&mut s, NODES + 0x1C, 64);
        put_f(&mut s, NODES + 64 + 0x24, 500.0);
        // DMA 1: NEXT over one block (0x10 + 0x70 = 0x80).
        put(&mut s, DMA, 0x2000_0000);
        put(&mut s, DMA + 4, 0x2000_0080);
        put(&mut s, DMA + 8, 0x0300_0080);
        put(&mut s, DMA + 12, 0x0200_01c0);
        let mut at = DMA + 0x80;
        put(&mut s, at, 0x1000_0003);
        put(&mut s, at + 8, 0x1000_0000);
        put(&mut s, at + 12, 0x6c03_003c);
        for (i, v) in [
            127.0, 127.0, 127.0, 0.0, 178.0, 178.0, 178.0, 128.0, 255.0, 255.0, 255.0, 0.0,
        ]
        .into_iter()
        .enumerate()
        {
            put_f(&mut s, at + 16 + 4 * i, v);
        }
        at += 64;
        // Matrix REF: block qword 1 -> VU slot 4.
        put(&mut s, at, 0x3000_0007);
        put(&mut s, at + 4, 0x2000_0010);
        put(&mut s, at + 8, 0x1000_0000);
        put(&mut s, at + 12, 0x6c07_0004);
        at += 16;
        // Geometry REF over the whole section.
        put(&mut s, at, 0x3000_0000 | (GEO_LEN / 16) as u32);
        put(&mut s, at + 4, 0x3000_0000);
        assert_eq!(at + 16, DMA + DMA_LEN);
        // Geometry: rigid strip of 3 vertices, MSCALF, then a weighted one.
        let mut g = GEOMETRY;
        put(&mut s, g, 0x6c01_8000);
        put(&mut s, g + 4, 0x8003);
        put(&mut s, g + 8, 0x301e_4000);
        put(&mut s, g + 12, 0x412);
        put(&mut s, g + 16, 4);
        put(&mut s, g + 20, 0x6909_8001);
        g += 24;
        for v in 0..3i16 {
            let base = g + 18 * v as usize;
            put_h(&mut s, base, 100 * v);
            put_h(&mut s, base + 2, 200);
            put_h(&mut s, base + 4, 1);
            put_h(&mut s, base + 10, 32767);
            put_h(&mut s, base + 12, v);
            put_h(&mut s, base + 14, 2 * v);
            put_h(&mut s, base + 16, 3 * v);
        }
        g += 54 + 2;
        put(&mut s, g, 0x1500_0000);
        g += 4;
        // Weighted: three vertices with k = 1, 2, 1 -> 15 triplets.
        put(&mut s, g, 0x6801_8000);
        put(&mut s, g + 4, 0x8003);
        put(&mut s, g + 8, 0x301e_4000);
        put(&mut s, g + 12, 0x8412);
        put(&mut s, g + 16, 0x690f_8001);
        let mut v = g + 20;
        for k in [1i16, 2, 1] {
            put_h(&mut s, v + 4, k);
            v += 6;
            for j in 0..k {
                put_h(&mut s, v, 4);
                put_h(&mut s, v + 2, if k == 2 { 2048 } else { 4096 });
                put_h(&mut s, v + 4, j);
                v += 18;
            }
        }
        assert_eq!(v - (g + 20), 90);
        assert!(g + 20 + 92 <= GEOMETRY + GEO_LEN);
        // Texture table: count 0, refs 16; relocation list terminators.
        put(&mut s, TEXTAB + 4, 16);
        put(&mut s, TEXTAB + 16, 0xffff_ffff);
        put(&mut s, TEXTAB + 32, 0xffff_ffff);
        // MOT (motion 0) after the RH0: two nodes, translation + rotation.
        s[MOT..MOT + 4].copy_from_slice(b"MOT\x1a");
        put(&mut s, MOT + 4, MOT_LEN as u32);
        s[MOT + 0x10..MOT + 0x12].copy_from_slice(&2u16.to_le_bytes());
        put(&mut s, MOT + 0x14, 0x20);
        put(&mut s, MOT + 0x18, 0x30);
        put(&mut s, MOT + 0x1C, 0xffff_ffff);
        put(&mut s, MOT + 0x20, 0x40);
        put(&mut s, MOT + 0x24, 4);
        s[MOT + 0x28..MOT + 0x2A].copy_from_slice(&3u16.to_le_bytes());
        s[MOT + 0x2A..MOT + 0x2C].copy_from_slice(&0x42u16.to_le_bytes());
        s[MOT + 0x30..MOT + 0x34].copy_from_slice(&[0xff; 4]);
        // Node 1 rotation track with three keys at 0x60.
        put(&mut s, MOT + 0x50 + 4, 0x60);
        put(&mut s, MOT + 0x50 + 12, 3);
        for (k, (frame, z)) in [(0u32, 0i32), (2, 0x4000), (3, 0x4000)]
            .into_iter()
            .enumerate()
        {
            put(&mut s, MOT + 0x60 + 16 * k, frame);
            put(&mut s, MOT + 0x60 + 16 * k + 12, z as u32);
        }
        // Sound cues: one entry, then the terminator.
        s[CUES..CUES + 2].copy_from_slice(&(2u16 | (15 << 4)).to_le_bytes());
        s[CUES + 2..CUES + 4].copy_from_slice(&7u16.to_le_bytes());
        s[CUES + 4..CUES + 6].copy_from_slice(&0xfu16.to_le_bytes());
        s
    }

    #[test]
    fn synthetic_slot_decodes_nodes_draws_strips_motions_and_cues() {
        let slot = synthetic_slot();
        let model = decode_pal_monster_slot(&slot, 5).unwrap();
        assert_eq!((model.card, model.archive_offset), (5, 5 * 0x10_0000));
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(
            (model.nodes[0].child, model.nodes[1].matrix_block),
            (Some(1), 1)
        );
        assert_eq!(model.matrix_blocks, 1);
        let draw = &model.passes[0].draws[0];
        assert_eq!(
            draw.matrix_slots,
            [MonsterMatrixSlot {
                vu_address: 4,
                matrix_block: 1
            }]
        );
        assert_eq!(draw.material().unwrap()[4], 178.0);
        assert!(draw.gs_registers.is_none() && draw.texture_index.is_none());
        assert_eq!(draw.strips.len(), 2);
        let rigid = &draw.strips[0];
        assert!(!rigid.skinned && rigid.rigid_matrix_vu == Some(4) && rigid.prim == 0x3c);
        assert_eq!(rigid.vertices[2].texture_raw, [200, 200, 1]);
        assert_eq!(rigid.vertices[2].influences[0].position_raw, [2, 4, 6]);
        assert_eq!(rigid.vertices[0].influences[0].normal_raw, [0, 0, 32767]);
        let weighted = &draw.strips[1];
        assert!(weighted.skinned);
        assert_eq!(
            weighted
                .vertices
                .iter()
                .map(|v| v.influences.len())
                .collect::<Vec<_>>(),
            [1, 2, 1]
        );
        assert_eq!(weighted.vertices[1].influences[1].lane_raw, Some(1));
        assert_eq!(weighted.vertices[1].influences[0].weight_raw, 2048);
        let motion = model.motion(MONSTER_MOTION_IDLE).unwrap();
        assert_eq!((motion.length, motion.kind, motion.node_count), (4, 3, 2));
        assert!(motion.tracks[0].rotation.is_none());
        assert_eq!(
            motion.tracks[1].rotation.as_ref().unwrap()[1],
            MonsterAngleKey(2, [0, 0, 0x4000])
        );
        assert_eq!(
            model.sound_cues,
            [MonsterSoundCue {
                motion: 2,
                frame: 15,
                sound: 7
            }]
        );
        let json = serde_json::to_string(&model).unwrap();
        let back: MonsterModel = serde_json::from_str(&json).unwrap();
        assert_eq!(back, model);
    }

    #[test]
    fn slot_rejects_bad_lengths_flags_and_links() {
        let slot = synthetic_slot();
        assert!(decode_pal_monster_slot(&slot[..1000], 0).is_err());
        assert!(decode_pal_monster_slot(&slot, 683).is_err());
        let mut bad = slot.clone();
        put(&mut bad, 4, 0x8020_0101);
        assert!(matches!(
            decode_pal_monster_slot(&bad, 0),
            Err(Error::Unsupported(_))
        ));
        let mut cycle = slot.clone();
        put(&mut cycle, 0xB0 + 64 + 0x2C, 64);
        assert!(decode_pal_monster_slot(&cycle, 0).is_err());
        let mut influence = slot.clone();
        // A weighted influence that names a VU slot never uploaded.
        put_h(&mut influence, FIRST_INFLUENCE, 11);
        assert!(decode_pal_monster_slot(&influence, 0).is_err());
        // 64 cue entries without the 0xF terminator must fail.
        let mut cues = slot;
        for i in 0..64 {
            cues[CUES + 4 * i..CUES + 4 * i + 2].copy_from_slice(&0x10u16.to_le_bytes());
        }
        assert!(decode_pal_monster_slot(&cues, 0).is_err());
    }

    #[test]
    fn sine_table_requires_the_fingerprinted_executable() {
        assert!(matches!(
            parse_pal_sine_table(&[0; 64]),
            Err(Error::UnsupportedExecutable { .. })
        ));
    }
}
