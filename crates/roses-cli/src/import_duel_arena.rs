//! `roses import-duel-arena`: the battle scene's arena stages from the
//! user's ISO into a new private directory: MODEL.MRG bank block 22 (the
//! shared stage piece, object 7) and blocks 23 + terrain (the terrain's
//! stage, objects 8..), decoded with the campaign map's RH0 grammar
//! (`decode_pal_model_bank`), their textures as PNG, the STAGECAM.MRG camera
//! paths (one CMOT per terrain, slice table EE 0x2913F0) copied verbatim,
//! and the executable's per-terrain environment records (EE 0x290910 +
//! terrain * 0xC0) as raw words. Records the grammar does not cover are
//! listed, never guessed. Nothing here may be distributed.
//!
//! Traced in the project notes: the
//! battle state 0x39 phase 2 loads 0x1E91B0(1, 0) (block 22) and
//! 0x1E91B0(2, terrain) (block 23 + terrain); the terrain is the battle
//! cell's.
use crate::import_campaign_map::{map_rgba, BANK_TABLE_EE};
use crate::{emit_json, open_pal, write_png, Result};
use roses_client::campaign_map::EE_TO_FILE;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const SCHEMA: &str = "roses-duel-arena-import-1";
const MODEL_ISO_PATH: &str = "DATA/MODEL.MRG";
const STAGECAM_ISO_PATH: &str = "DATA/STAGECAM.MRG";
/// The attack effect programs (one 0x8000 record per card); with the
/// executable, data for the hand ports (`roses-fx`): no original code runs.
const BMODULE_ISO_PATH: &str = "DATA/BMODULE.MRG";
/// The card / duel effect programs (type-0 tasks; the duel start's leader
/// appearance c620 is run from it).
const CMODULE_ISO_PATH: &str = "DATA/CMODULE.MRG";
/// Bank block of the shared stage piece and of terrain 0's stage.
pub const SHARED_BLOCK: u32 = 22;
pub const FIRST_TERRAIN_BLOCK: u32 = 23;
pub const TERRAINS: u32 = 10;
/// StageCam slice offsets (u32 per terrain, then the end).
pub const STAGECAM_TABLE_EE: u32 = 0x0029_13F0;
/// Per-terrain environment records (0xC0 bytes each).
pub const ENVIRONMENT_EE: u32 = 0x0029_0910;
pub const ENVIRONMENT_BYTES: u32 = 0xC0;
/// Per-card u16 tables read by the arena's attack phase
/// (ARENA-ATTACK.md): unit size (close-up distance, 0x1DF4D0) and camera
/// flags (0x8000 / 0x4000 random mirror); 684 entries each (the flag table
/// ends where the size table starts).
pub const UNIT_SIZE_EE: u32 = 0x002C_EC28;
pub const UNIT_FLAGS_EE: u32 = 0x002C_E6D0;
pub const UNIT_TABLE_ENTRIES: u32 = 684;

#[derive(Serialize)]
struct FileRef {
    path: String,
    sha256: String,
    bytes: u64,
}
#[derive(Serialize)]
struct TextureRef {
    index: usize,
    path: String,
    sha256: String,
    width: u32,
    height: u32,
}
#[derive(Serialize)]
struct ModelRef {
    slot: u16,
    role: &'static str,
    ordinal: u16,
    file_offset: u64,
    record_sha256: String,
    node_count: u8,
    draws: usize,
    strips: usize,
    vertices: usize,
    textures: Vec<TextureRef>,
}
#[derive(Serialize)]
struct BlockRef {
    block: u32,
    /// None for the shared block, else the terrain code (0 forest ..).
    terrain: Option<u32>,
    block_offset: u32,
    block_end: u32,
    geometry: FileRef,
    models: Vec<ModelRef>,
    unsupported: Vec<roses_assets::UnsupportedCampaignMapRecord>,
}

fn u32_at(executable: &[u8], ee: u32) -> Result<u32> {
    let at = ee
        .checked_sub(EE_TO_FILE)
        .ok_or("EE address below the executable image")? as usize;
    Ok(u32::from_le_bytes(
        executable
            .get(at..at + 4)
            .ok_or_else(|| format!("executable is too short for EE {ee:#x}"))?
            .try_into()
            .unwrap(),
    ))
}

fn file_ref(output: &Path, path: &str) -> Result<FileRef> {
    let bytes = fs::read(output.join(path))?;
    Ok(FileRef {
        path: path.into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes: bytes.len() as u64,
    })
}

pub fn run(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "duel arena output must be a new directory; existing imports are never overwritten"
                .into(),
        );
    }
    let (mut disc, report) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let rules = roses_assets::parse_pal_rules(&executable)?;
    let archive = disc.read_file(MODEL_ISO_PATH, 32 * 1024 * 1024)?;
    let stagecam = disc.read_file(STAGECAM_ISO_PATH, 64 * 1024)?;
    if stagecam.get(..4) != Some(b"CMOT") {
        return Err("STAGECAM.MRG does not start with a CMOT record".into());
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    fs::create_dir(output.join("textures"))?;
    let mut blocks = Vec::new();
    let mut archive_sha256 = String::new();
    for (index, block) in (SHARED_BLOCK..FIRST_TERRAIN_BLOCK + TERRAINS).enumerate() {
        let block_offset = u32_at(&executable, BANK_TABLE_EE + 4 * block)?;
        let block_end = u32_at(&executable, BANK_TABLE_EE + 4 * (block + 1))?;
        let first_slot = if block == SHARED_BLOCK { 7 } else { 8 };
        let geometry = roses_assets::decode_pal_model_bank(
            &archive,
            block_offset as usize,
            block_end as usize,
            first_slot,
        )?;
        archive_sha256.clone_from(&geometry.archive_sha256);
        let name = if block == SHARED_BLOCK {
            "shared".to_string()
        } else {
            format!("terrain-{}", block - FIRST_TERRAIN_BLOCK)
        };
        let mut models = Vec::new();
        for model in &geometry.models {
            let mut textures = Vec::new();
            for t in 0..model.textures.len() {
                let texture = roses_assets::decode_pal_campaign_map_texture(&archive, model, t)?;
                let path = format!("textures/{name}-slot-{}-t{t}.png", model.slot);
                write_png(
                    &output.join(&path),
                    texture.width,
                    texture.height,
                    &map_rgba(&texture)?,
                )?;
                let file = file_ref(output, &path)?;
                textures.push(TextureRef {
                    index: t,
                    path,
                    sha256: file.sha256,
                    width: texture.width,
                    height: texture.height,
                });
            }
            models.push(ModelRef {
                slot: model.slot,
                role: if block == SHARED_BLOCK { "shared stage piece" } else { "terrain stage" },
                ordinal: model.ordinal,
                file_offset: model.file_offset,
                record_sha256: model.source_sha256.clone(),
                node_count: model.node_count,
                draws: model.draws.len(),
                strips: model.strips.len(),
                vertices: model.strips.iter().map(|s| s.vertices.len()).sum(),
                textures,
            });
        }
        let geometry_path = format!("{name}.geometry.json");
        emit_json(&geometry, Some(&output.join(&geometry_path)))?;
        blocks.push(BlockRef {
            block,
            terrain: (block != SHARED_BLOCK).then(|| block - FIRST_TERRAIN_BLOCK),
            block_offset,
            block_end,
            geometry: file_ref(output, &geometry_path)?,
            models,
            unsupported: geometry.unsupported.clone(),
        });
        let _ = index;
    }
    fs::write(output.join("STAGECAM.MRG"), &stagecam)?;
    let bmodule = disc.read_file(BMODULE_ISO_PATH, 32 * 1024 * 1024)?;
    if bmodule.get(..4) != Some(b"MWo3") {
        return Err("BMODULE.MRG does not start with an MWo3 record".into());
    }
    fs::write(output.join("BMODULE.MRG"), &bmodule)?;
    let cmodule = disc.read_file(CMODULE_ISO_PATH, 40 * 1024 * 1024)?;
    if cmodule.get(..4) != Some(b"MWo3") {
        return Err("CMODULE.MRG does not start with an MWo3 record".into());
    }
    fs::write(output.join("CMODULE.MRG"), &cmodule)?;
    fs::write(output.join("SLES_515.79"), &executable)?;
    let stagecam_slices: Vec<u32> = (0..=TERRAINS)
        .map(|t| u32_at(&executable, STAGECAM_TABLE_EE + 4 * t))
        .collect::<Result<_>>()?;
    let environments: Vec<Vec<u32>> = (0..TERRAINS)
        .map(|t| {
            (0..ENVIRONMENT_BYTES / 4)
                .map(|w| u32_at(&executable, ENVIRONMENT_EE + t * ENVIRONMENT_BYTES + 4 * w))
                .collect::<Result<Vec<u32>>>()
        })
        .collect::<Result<_>>()?;
    let u16_table = |ee: u32| -> Result<Vec<u16>> {
        (0..UNIT_TABLE_ENTRIES / 2)
            .map(|i| u32_at(&executable, ee + 4 * i))
            .collect::<Result<Vec<u32>>>()
            .map(|w| w.iter().flat_map(|v| [*v as u16, (*v >> 16) as u16]).collect())
    };
    let unit_sizes = u16_table(UNIT_SIZE_EE)?;
    let unit_flags = u16_table(UNIT_FLAGS_EE)?;
    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(
        &serde_json::json!({
            "schema": SCHEMA,
            "importer_version": env!("CARGO_PKG_VERSION"),
            "profile": roses_disc::PAL_PROFILE_ID,
            "source": {
                "iso_sha256": report.iso_sha256,
                "executable_sha256": rules.executable_sha256,
                "model_archive": {"iso_path": MODEL_ISO_PATH, "sha256": archive_sha256, "bytes": archive.len()},
            },
            "blocks": blocks,
            "stagecam": file_ref(output, "STAGECAM.MRG")?,
            "effects": {"bmodule": file_ref(output, "BMODULE.MRG")?, "cmodule": file_ref(output, "CMODULE.MRG")?, "executable": file_ref(output, "SLES_515.79")?},
            "stagecam_slices": {"ee": STAGECAM_TABLE_EE, "offsets": stagecam_slices},
            "environments": {"ee": ENVIRONMENT_EE, "stride": ENVIRONMENT_BYTES, "raw_words": environments},
            "unit_tables": {"size": {"ee": UNIT_SIZE_EE, "values": unit_sizes}, "flags": {"ee": UNIT_FLAGS_EE, "values": unit_flags}},
            "texture_encoding": "16-bit CLUT: PNG RGB = GS expansion (5-bit value << 3), PNG alpha 255 = CLUT alpha bit; 32-bit CLUT: RGB bytes as stored, PNG alpha = GS alpha * 255 / 128",
            "not_included": [
                "the environment records' meaning (raw words only)",
                "stage records the RH0 grammar does not cover (listed per block)"
            ],
            "complete_game_import": false,
            "rendering_validated": false,
        }),
        Some(&output.join("manifest.json")),
    )?;
    Ok(())
}
