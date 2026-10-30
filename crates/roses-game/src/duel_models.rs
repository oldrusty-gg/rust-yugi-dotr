//! Duel unit models (MONSTER.MRG) for the optional renderer; no GPU code.
//!
//! Input is the private output of `roses import-duel-models` (manifest,
//! compact `roses_assets::MonsterModel` JSON per card, GS-byte PNGs),
//! hash-checked on load. Pose, skinning and the per-vertex lighting come from
//! `roses_client::monster_pose` (ports of 0x210050 / 0x2100D0 / 0x210480,
//! object matrix 0x1EF8F0, motion time 0x1EE6E0, verified against 131
//! savestate objects and the round-2 GS dump in private tests). This module
//! places a unit on a board cell, keeps its motion clock and turns every
//! frame into triangle lists per GS draw state.
//!
//! GS rules (private model research, NOTES.md): TFX MODULATE, TCC from the
//! texture's stored TEX0, C_16 texel alpha bit -> 128/0 (TEXA), bilinear,
//! CLAMP per draw, Z test GREATER, no culling, no fog; the opaque pass
//! (DMA 1) draws with ABE 0 and the TEST alpha test, the second pass (DMA 2)
//! blends with ALPHA 0x44 (normal), 0x48 (additive) or 0x42 (subtractive).
//! Flat-shaded strips (PRIM IIP 0) take the colour of each triangle's last
//! vertex. Placement: board cell (c, r) -> ((c - 3) 50, 50, (r - 3) 50), Y
//! 0x8000 (side 0) or 0 (side 1), recentred (obj+0x1FF = 32), grow
//! clamp(40 / node-0 radius, 0.3, 1).
//!
//! Draw order is the original one: the opaque pass, then the blended pass,
//! draws in DMA order and strips in packet order (`draw_groups` only merges
//! consecutive strips; `draw_order_key` pins Bevy's sorted phase to it).
//!
//! Not modelled: motion cross-fades, summon grow-in, the blended pass's Z
//! write (kept on), the cursor unit's offscreen composite, near-camera fade,
//! textures the RH2 decoder does not support (their draws are skipped and
//! reported).
use crate::camera::Mat4;
use crate::{Result, bounded_read, hash_valid, validate_relative_path};
use roses_assets::{
    MonsterModel, MonsterTextureRef, PAL_MONSTER_ARCHIVE_BYTES, PAL_MONSTER_SLOT_BYTES,
    PAL_MONSTER_SLOT_COUNT,
};
use roses_client::monster_pose::{
    self, BOARD_UNIT_LIGHT, MotionClock, SineTable, UnitObject, object_matrix,
    skin_model,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MANIFEST_SCHEMA: &str = "roses-duel-models-import-1";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MODEL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TEXTURE_BYTES: u64 = 4 * 1024 * 1024;

// ---------------------------------------------------------------- manifest
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub source: ManifestSource,
    pub sine_table: SineTableRef,
    pub unit_dither_matrix: DitherRef,
    pub models: Vec<ModelRef>,
    /// The verbatim MONSTER.MRG with each slot's SHA-256 (imports from
    /// 2 Oct 2026 on): cards outside `models` are decoded from it.
    #[serde(default)]
    pub archive: Option<ArchiveFile>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ArchiveFile {
    pub path: String,
    pub slot_sha256: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ManifestSource {
    pub iso_sha256: String,
    pub executable_sha256: String,
    pub monster_archive: ArchiveRef,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ArchiveRef {
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct SineTableRef {
    pub values: Vec<f32>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct DitherRef {
    pub matrix: [[i8; 4]; 4],
}
#[derive(Clone, Debug, Deserialize)]
pub struct ModelRef {
    pub card: u16,
    pub path: String,
    pub sha256: String,
    pub slot_sha256: String,
    pub textures: Vec<TextureRef>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct TextureRef {
    pub kind: String,
    pub index: usize,
    pub path: Option<String>,
    pub sha256: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub unsupported: Option<String>,
}

/// A model texture: PNG bytes from the import, or GS texel bytes decoded
/// from the archive at load.
#[derive(Clone, Debug)]
pub enum TextureBytes {
    Png(Vec<u8>),
    Rgba {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
}

/// One card's model with its hash-checked texture bytes (model textures
/// only).
#[derive(Clone, Debug)]
pub struct LoadedModel {
    pub manifest_sha256: String,
    pub source: ManifestSource,
    pub model: MonsterModel,
    pub model_sha256: String,
    /// Model texture index -> (manifest path or archive label, bytes).
    pub textures: BTreeMap<usize, (String, TextureBytes)>,
    /// Model textures without a PNG, with the importer's reason.
    pub missing_textures: BTreeMap<usize, String>,
    pub sine: SineTable,
    pub dither: [[i8; 4]; 4],
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn child(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("{relative}: {e}"))?;
    if !path.starts_with(root) {
        return Err("Model file resolves outside the import directory".into());
    }
    Ok(path)
}

/// Load and hash-check one card of a models import.
pub fn load_model(manifest_path: &Path, card: u16) -> Result<LoadedModel> {
    let bytes = bounded_read(manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Models manifest: {e}"))?;
    if manifest.schema != MANIFEST_SCHEMA
        || ![
            &manifest.source.iso_sha256,
            &manifest.source.executable_sha256,
            &manifest.source.monster_archive.sha256,
        ]
        .into_iter()
        .all(|h| hash_valid(h))
    {
        return Err("Unsupported models manifest schema or fingerprints".into());
    }
    if manifest
        .unit_dither_matrix
        .matrix
        .iter()
        .flatten()
        .any(|v| !(-4..=3).contains(v))
    {
        return Err("Models manifest dither matrix is out of range".into());
    }
    let sine = SineTable::new(manifest.sine_table.values.clone())?;
    let root = manifest_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let Some(reference) = manifest.models.iter().find(|m| m.card == card) else {
        let archive = manifest
            .archive
            .as_ref()
            .ok_or_else(|| format!("The models import has no card {card}"))?;
        let (model, textures, missing_textures) = decode_from_archive(&root, archive, card)?;
        return Ok(LoadedModel {
            manifest_sha256: sha(&bytes),
            model_sha256: model.slot_sha256.clone(),
            source: manifest.source,
            model,
            textures,
            missing_textures,
            sine,
            dither: manifest.unit_dither_matrix.matrix,
        });
    };
    let model_bytes = bounded_read(&child(&root, &reference.path)?, MAX_MODEL_BYTES)?;
    if !hash_valid(&reference.sha256) || sha(&model_bytes) != reference.sha256 {
        return Err(format!("Model fingerprint mismatch: {}", reference.path));
    }
    let model: MonsterModel =
        serde_json::from_slice(&model_bytes).map_err(|e| format!("{}: {e}", reference.path))?;
    if model.card != card || model.slot_sha256 != reference.slot_sha256 {
        return Err("Model record does not match the manifest".into());
    }
    let mut textures = BTreeMap::new();
    let mut missing_textures = BTreeMap::new();
    for texture in reference.textures.iter().filter(|t| t.kind == "model") {
        match (
            &texture.path,
            &texture.sha256,
            texture.width,
            texture.height,
        ) {
            (Some(path), Some(hash), Some(w), Some(h)) => {
                let png = bounded_read(&child(&root, path)?, MAX_TEXTURE_BYTES)?;
                if !hash_valid(hash)
                    || sha(&png) != *hash
                    || png.get(..8) != Some(b"\x89PNG\r\n\x1a\n")
                    || w == 0
                    || h == 0
                    || w > 1024
                    || h > 1024
                {
                    return Err(format!("Texture mismatch: {path}"));
                }
                textures.insert(texture.index, (path.clone(), TextureBytes::Png(png)));
            }
            _ => {
                missing_textures.insert(
                    texture.index,
                    texture
                        .unsupported
                        .clone()
                        .unwrap_or_else(|| "no PNG in the import".into()),
                );
            }
        }
    }
    // An import made before the decoder learned a texture layout (the
    // 64x128/64x256 banded P_8 rectangles, 128-wide rectangles) has no PNG
    // for it: decode those from the import's verbatim MONSTER.MRG copy,
    // which is the same slot (checked by its SHA-256).
    let mut model = model;
    if !missing_textures.is_empty()
        && let Some(archive) = manifest.archive.as_ref()
        && let Ok((fresh, decoded, _)) = decode_from_archive(&root, archive, card)
        && fresh.slot_sha256 == model.slot_sha256
        && fresh.textures.len() == model.textures.len()
    {
        for (index, texture) in decoded {
            if missing_textures.remove(&index).is_some() {
                model.textures[index].record = fresh.textures[index].record.clone();
                model.textures[index].unsupported = None;
                textures.insert(index, texture);
            }
        }
    }
    Ok(LoadedModel {
        manifest_sha256: sha(&bytes),
        source: manifest.source,
        model,
        model_sha256: reference.sha256.clone(),
        textures,
        missing_textures,
        sine,
        dither: manifest.unit_dither_matrix.matrix,
    })
}

/// Model textures of [`LoadedModel`] and the missing ones with reasons.
type Textures = (
    BTreeMap<usize, (String, TextureBytes)>,
    BTreeMap<usize, String>,
);

/// The 1 MiB MONSTER.MRG slot of `card` from the import's verbatim copy,
/// checked against its recorded SHA-256.
fn read_slot(root: &Path, archive: &ArchiveFile, card: u16) -> Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let index = usize::from(card);
    let expected = archive
        .slot_sha256
        .get(index)
        .ok_or_else(|| format!("MONSTER.MRG has no slot for card {card}"))?;
    if archive.slot_sha256.len() != PAL_MONSTER_SLOT_COUNT || !hash_valid(expected) {
        return Err("Unsupported MONSTER.MRG slot list".into());
    }
    let path = child(root, &archive.path)?;
    let mut file = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", archive.path))?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    if length != PAL_MONSTER_ARCHIVE_BYTES {
        return Err("MONSTER.MRG copy has an unsupported length".into());
    }
    file.seek(SeekFrom::Start(
        index as u64 * PAL_MONSTER_SLOT_BYTES as u64,
    ))
    .map_err(|e| e.to_string())?;
    let mut slot = vec![0; PAL_MONSTER_SLOT_BYTES];
    file.read_exact(&mut slot).map_err(|e| e.to_string())?;
    if sha(&slot) != *expected {
        return Err(format!("MONSTER.MRG slot {card} fingerprint mismatch"));
    }
    Ok(slot)
}

/// The attack camera of `card`'s model: the slot's first chained CMOT
/// record (object +0x16C, `cmot_0x16c`), read from the import's MONSTER.MRG
/// copy; `None` when the slot has none (the original then keeps the
/// arena camera [F 0x1F1EE0]).
pub fn attack_camera(manifest_path: &Path, card: u16) -> Result<Option<Vec<u8>>> {
    Ok(unit_cameras(manifest_path, card)?.0)
}

/// Both chained CMOT records of `card`'s slot: (object +0x16C, the attack
/// camera; object +0x170, the victory camera).
pub fn unit_cameras(manifest_path: &Path, card: u16) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>)> {
    let bytes = bounded_read(manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Models manifest: {e}"))?;
    let root = manifest_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let archive = manifest
        .archive
        .as_ref()
        .ok_or("The models import has no MONSTER.MRG copy (attack cameras need it)")?;
    let slot = read_slot(&root, archive, card)?;
    let model = roses_assets::decode_pal_monster_slot(&slot, card).map_err(|e| e.to_string())?;
    let get = |kind: &str| -> Result<Option<Vec<u8>>> {
        let Some(record) = model.camera_records.iter().find(|r| r.kind == kind) else {
            return Ok(None);
        };
        let start = record.slot_offset as usize;
        let cmot = slot
            .get(start..start + record.byte_len as usize)
            .ok_or("Camera record outside its slot")?;
        if sha(cmot) != record.sha256 || cmot.get(..4) != Some(b"CMOT") {
            return Err(format!("Card {card}: camera record mismatch"));
        }
        Ok(Some(cmot.to_vec()))
    };
    Ok((get("cmot_0x16c")?, get("cmot_0x170")?))
}

/// One effect texture of a monster's slot (obj+0x144 + 4 * slot, the
/// attack effects' type-5 textures): the verbatim RH2 record and, when the
/// decoder supports it, its pixels (RGBA, alpha = GS alpha * 255 / 128).
#[derive(Clone, Debug)]
pub struct EffectTexture {
    pub rh2: Vec<u8>,
    pub image: Option<roses_assets::RgbaImage>,
}

/// `card`'s effect textures by effect slot (0..8), read from the import's
/// MONSTER.MRG copy.
pub fn effect_textures(manifest_path: &Path, card: u16) -> Result<Vec<Option<EffectTexture>>> {
    let bytes = bounded_read(manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| format!("Models manifest: {e}"))?;
    let root = manifest_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let archive = manifest.archive.as_ref().ok_or("The models import has no MONSTER.MRG copy (effect textures need it)")?;
    let slot = read_slot(&root, archive, card)?;
    let model = roses_assets::decode_pal_monster_slot(&slot, card).map_err(|e| e.to_string())?;
    let mut out: Vec<Option<EffectTexture>> = vec![None; 8];
    for (i, t) in model.effect_textures.iter().enumerate() {
        let start = t.slot_offset as usize;
        let Some(rh2) = slot.get(start..start + t.byte_len as usize) else { continue };
        let image = roses_assets::decode_pal_monster_texture(&slot, &model, MonsterTextureRef::Effect(i))
            .ok()
            .and_then(|t| crate::duel_bmodule::indexed_rgba(&t));
        if let Some(o) = out.get_mut(usize::from(t.effect_slot)) {
            *o = Some(EffectTexture { rh2: rh2.to_vec(), image });
        }
    }
    Ok(out)
}

/// Decode `card` from the import's verbatim MONSTER.MRG: the slot is read
/// at card x 1 MiB and must match its recorded SHA-256; textures without a
/// record are missing, as the importer reports them.
fn decode_from_archive(
    root: &Path,
    archive: &ArchiveFile,
    card: u16,
) -> Result<(
    MonsterModel,
    BTreeMap<usize, (String, TextureBytes)>,
    BTreeMap<usize, String>,
)> {
    let slot = read_slot(root, archive, card)?;
    let model = roses_assets::decode_pal_monster_slot(&slot, card).map_err(|e| e.to_string())?;
    let (mut textures, mut missing): Textures = (BTreeMap::new(), BTreeMap::new());
    for (i, texture) in model.textures.iter().enumerate() {
        if texture.record.is_none() {
            missing.insert(
                i,
                texture
                    .unsupported
                    .clone()
                    .unwrap_or_else(|| "no texture record".into()),
            );
            continue;
        }
        match roses_assets::decode_pal_monster_texture(&slot, &model, MonsterTextureRef::Model(i)) {
            Ok(t) => {
                let pixels = roses_assets::monster_gs_texels(&t);
                textures.insert(
                    i,
                    (
                        format!("MONSTER.MRG card {card} texture {i}"),
                        TextureBytes::Rgba {
                            width: t.width,
                            height: t.height,
                            pixels,
                        },
                    ),
                );
            }
            Err(error) => {
                missing.insert(i, error.to_string());
            }
        }
    }
    Ok((model, textures, missing))
}

// ---------------------------------------------------------------- GS state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// ALPHA 0x44: (Cs - Cd) As + Cd.
    Normal,
    /// ALPHA 0x48: Cs As + Cd.
    Additive,
    /// ALPHA 0x42: Cd - Cs As.
    Subtractive,
}

/// Fixed-function state of one GPU draw group.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawState {
    pub textured: bool,
    pub clamp_u: bool,
    pub clamp_v: bool,
    pub linear: bool,
    /// ATST GREATER reference when the alpha test is on (AFAIL KEEP).
    pub alpha_greater_than: Option<u8>,
    /// TEX0 TCC: texture alpha used (else vertex alpha only).
    pub texture_alpha: bool,
    pub blend: Option<Blend>,
}

/// Decode the draw's GS registers for strips with PRIM `prim`. `tex0_high`
/// is RH2 header word 13 of the selected texture (stored TEX0 bits 32-63).
pub fn draw_state(
    registers: &[[u64; 2]],
    prim: u16,
    tex0_high: Option<u32>,
) -> std::result::Result<DrawState, String> {
    let register = |address: u64| {
        registers
            .iter()
            .find(|pair| pair[1] == address)
            .map(|pair| pair[0])
            .ok_or_else(|| format!("GS register {address:#x} missing"))
    };
    let clamp = register(0x08)?;
    let test = register(0x47)?;
    let alpha = register(0x42)?;
    let tex1 = register(0x14)?;
    let (wms, wmt) = (clamp & 3, (clamp >> 2) & 3);
    if wms > 1 || wmt > 1 {
        return Err(format!("region clamp CLAMP {clamp:#x}"));
    }
    let (mmag, mmin) = ((tex1 >> 5) & 1, (tex1 >> 6) & 7);
    if mmag != mmin || mmin > 1 {
        return Err(format!("mipmapped or mixed filters TEX1 {tex1:#x}"));
    }
    let (ate, atst, aref, afail) = (
        test & 1,
        (test >> 1) & 7,
        (test >> 4) & 255,
        (test >> 12) & 3,
    );
    let (date, zte, ztst) = ((test >> 14) & 1, (test >> 16) & 1, (test >> 17) & 3);
    if date != 0 || zte != 1 || ztst != 3 || test >> 19 != 0 {
        return Err(format!("TEST {test:#x}"));
    }
    let alpha_greater_than = match (ate, atst, afail) {
        (0, _, _) | (1, 1, _) => None,
        (1, 6, 0) => Some(aref as u8),
        _ => return Err(format!("alpha test TEST {test:#x}")),
    };
    let textured = prim & 0x10 != 0;
    let texture_alpha = match tex0_high {
        Some(word) if textured => {
            if (word >> 3) & 3 != 0 {
                return Err("only TFX MODULATE is supported".into());
            }
            (word >> 2) & 1 == 1
        }
        None if textured => return Err("textured strips without a texture".into()),
        _ => false,
    };
    let blend = if prim & 0x40 != 0 {
        Some(match alpha & 0xff {
            0x44 => Blend::Normal,
            0x48 => Blend::Additive,
            0x42 => Blend::Subtractive,
            other => return Err(format!("ALPHA {other:#x}")),
        })
    } else {
        None
    };
    Ok(DrawState {
        textured,
        clamp_u: wms == 1,
        clamp_v: wmt == 1,
        linear: mmag == 1,
        alpha_greater_than,
        texture_alpha,
        blend,
    })
}

/// Strips of one RH0 draw that share a PRIM texture/blend mode.
#[derive(Clone, Debug)]
pub struct DrawGroup {
    pub pass: u8,
    pub draw: usize,
    pub strips: Vec<usize>,
    pub state: DrawState,
    /// Model texture index (PNG in `LoadedModel::textures`) when textured.
    pub texture: Option<usize>,
}

/// Group every drawable strip, in the original order (pass 0 then pass 1,
/// draws in DMA order, strips in packet order: only consecutive strips of
/// one draw with the same state share a group, so concatenating the groups
/// replays the original strip sequence). Draws that cannot be reproduced
/// are reported.
pub fn draw_groups(loaded: &LoadedModel) -> (Vec<DrawGroup>, Vec<String>) {
    let model = &loaded.model;
    let mut groups: Vec<DrawGroup> = Vec::new();
    let mut skipped = Vec::new();
    for pass in &model.passes {
        for (index, draw) in pass.draws.iter().enumerate() {
            let label = format!("pass {} draw {index}", pass.pass);
            let Some(registers) = &draw.gs_registers else {
                skipped.push(format!(
                    "{label}: no GS packet in its pass (state unresolved)"
                ));
                continue;
            };
            let texture = draw.texture_index.map(usize::from);
            let tex0_high = texture.and_then(|t| {
                model.textures[t]
                    .record
                    .as_ref()
                    .and_then(|r| r.raw_header_words.get(13).copied())
            });
            let first = groups.len();
            for (strip_index, strip) in draw.strips.iter().enumerate() {
                let textured = strip.prim & 0x10 != 0;
                if textured && texture.is_none_or(|t| !loaded.textures.contains_key(&t)) {
                    let reason = texture
                        .and_then(|t| loaded.missing_textures.get(&t).cloned())
                        .unwrap_or_else(|| "no texture selected".into());
                    let message = format!("{label}: texture unavailable ({reason})");
                    if !skipped.contains(&message) {
                        skipped.push(message);
                    }
                    continue;
                }
                let state = match draw_state(registers, strip.prim, tex0_high) {
                    Ok(state) => state,
                    Err(error) => {
                        let message = format!("{label}: {error}");
                        if !skipped.contains(&message) {
                            skipped.push(message);
                        }
                        continue;
                    }
                };
                // Only the latest group of this draw can take the strip.
                let own = groups.len() > first;
                if let Some(group) = groups.last_mut().filter(|g| own && g.state == state) {
                    group.strips.push(strip_index);
                } else {
                    groups.push(DrawGroup {
                        pass: pass.pass,
                        draw: index,
                        strips: vec![strip_index],
                        texture: textured.then_some(texture).flatten(),
                        state,
                    });
                }
            }
        }
    }
    (groups, skipped)
}

// ---------------------------------------------------------------- frames
/// Expanded triangle list of one group for one frame: world positions,
/// GS vertex colours (0..255, alpha 128 = 1.0) and S/T.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameMesh {
    pub positions: Vec<[f32; 3]>,
    pub colours: Vec<[f32; 4]>,
    pub uv: Vec<[f32; 2]>,
}

/// Triangles of one strip (every strip triangle is drawn: no culling).
/// Flat-shaded strips use the colour of each triangle's last vertex.
pub fn expand_strip(
    mesh: &mut FrameMesh,
    positions: &[[f64; 3]],
    colours: &[[u8; 4]],
    uv: &[[f32; 2]],
    flat: bool,
) {
    for i in 2..positions.len() {
        for corner in [i - 2, i - 1, i] {
            let colour = colours[if flat { i } else { corner }];
            mesh.positions.push(positions[corner].map(|v| v as f32));
            mesh.colours.push(colour.map(f32::from));
            mesh.uv.push(uv[corner]);
        }
    }
}

// ------------------------------------------------------------ draw order
/// Uniform scale of every group entity's transform. The model shader projects
/// world positions with its own matrix and never reads the entity transform,
/// which therefore only feeds Bevy's sorted (transparent) phase. That phase
/// draws in ascending view z of `transform * mesh AABB centre` (identity
/// camera: its z), so with translation z rising by one per group and this
/// scale the geometry moves a key by at most a quarter step and groups draw
/// in the original order at every frame.
pub const DRAW_ORDER_SCALE: f32 = 1.0 / 1_048_576.0;
/// Largest |coordinate| a group mesh may hold (0.25 / `DRAW_ORDER_SCALE`).
pub const DRAW_ORDER_EXTENT: f32 = 262_144.0;
/// Most groups one scene may hold (keys stay exact far beyond it).
pub const MAX_DRAW_GROUPS: usize = 4096;

/// Translation z of group `order`'s entity (in front of the identity
/// camera: -4096 for the first group, ascending by one).
pub fn draw_order_depth(order: usize) -> f32 {
    order as f32 - MAX_DRAW_GROUPS as f32
}

/// Bevy's sort key for group `order` whose mesh AABB centre has this z.
pub fn draw_order_key(order: usize, aabb_centre_z: f32) -> f32 {
    draw_order_depth(order) + DRAW_ORDER_SCALE * aabb_centre_z
}

/// Every mesh coordinate must be finite and within `DRAW_ORDER_EXTENT`.
pub fn check_draw_order_extent(frame: &[FrameMesh]) -> Result<()> {
    if frame.len() > MAX_DRAW_GROUPS {
        return Err(format!(
            "{} draw groups exceed {MAX_DRAW_GROUPS}",
            frame.len()
        ));
    }
    let outside = |v: &f32| !v.is_finite() || v.abs() > DRAW_ORDER_EXTENT;
    match frame
        .iter()
        .position(|m| m.positions.iter().flatten().any(outside))
    {
        Some(group) => Err(format!(
            "Group {group} has a position outside +-{DRAW_ORDER_EXTENT} (draw order key)"
        )),
        None => Ok(()),
    }
}

/// A unit model standing on a board cell, animated by its motion clock.
pub struct UnitScene {
    pub loaded: LoadedModel,
    pub motion: u8,
    pub object: UnitObject,
    pub world: Mat4,
    pub clock: MotionClock,
    /// Lighting brightness g (object +0x120 lane 2; 1.0 in the duel states).
    pub brightness: f64,
    pub groups: Vec<DrawGroup>,
    pub skipped: Vec<String>,
    /// The side's facing (0x8000 side 0, 0 side 1).
    pub base_yaw: i32,
    /// A motion change being blended: (previous motion, its time, the
    /// current motion's weight).
    pub blend: Option<(u8, f32, f32)>,
    /// The light the model is lit with (the board's, or an arena terrain's).
    pub light: roses_client::monster_pose::UnitLight,
}

impl UnitScene {
    pub fn new(
        loaded: LoadedModel,
        motion: u8,
        cell: (u8, u8),
        side: u8,
        brightness: f64,
    ) -> Result<Self> {
        // Sides 2 and 3: an N-player duel's E and W seats (a port addition).
        let sides = if cfg!(feature = "online") { 3 } else { 1 };
        if cell.0 > 6 || cell.1 > 6 || side > sides {
            return Err("Cell must be 0..=6,0..=6 and side 0 or 1".into());
        }
        let length = loaded
            .model
            .motion(motion)
            .ok_or_else(|| format!("Card {} has no motion {motion}", loaded.model.card))?
            .length;
        #[allow(unused_mut)]
        let mut object = UnitObject::board_unit(&loaded.model, cell.0, cell.1, side);
        #[cfg(feature = "online")]
        if side > 1 {
            object.rotation[1] = crate::duel_multi::model_yaw(side) as _;
        }
        let world = object_matrix(&loaded.model, &object, &loaded.sine);
        let (groups, skipped) = draw_groups(&loaded);
        if groups.is_empty() {
            return Err("No drawable group in this model".into());
        }
        Ok(Self {
            loaded,
            motion,
            object,
            world,
            clock: MotionClock::new(motion, length),
            brightness,
            groups,
            skipped,
            base_yaw: if side > 1 { side_model_yaw(side) } else if side == 0 { 0x8000 } else { 0 },
            blend: None,
            light: BOARD_UNIT_LIGHT,
        })
    }

    /// Meshes of every group at the clock's current (pre-update) time,
    /// checked against the draw-order key's extent.
    pub fn frame(&self) -> Result<Vec<FrameMesh>> {
        let frame = self.meshes()?;
        check_draw_order_extent(&frame)?;
        Ok(frame)
    }

    fn meshes(&self) -> Result<Vec<FrameMesh>> {
        let model = &self.loaded.model;
        let blend = self.blend.and_then(|(m, time, weight)| {
            model
                .motion(m)
                .map(|previous| roses_client::monster_pose::PoseBlend { previous, time, weight })
        });
        let pose = roses_client::monster_pose::evaluate_pose_blended(
            model,
            model.motion(self.motion),
            self.clock.time,
            blend,
            &self.loaded.sine,
        )?;
        let skinned = skin_model(
            model,
            &pose,
            &self.world,
            &self.light,
            self.brightness,
        )?;
        let lookup: BTreeMap<(u8, usize), &monster_pose::SkinnedDraw> =
            skinned.iter().map(|d| ((d.pass, d.draw), d)).collect();
        self.groups
            .iter()
            .map(|group| {
                let draw = lookup
                    .get(&(group.pass, group.draw))
                    .ok_or("Skinned draw missing")?;
                let mut mesh = FrameMesh::default();
                for &index in &group.strips {
                    let strip = &draw.strips[index];
                    expand_strip(
                        &mut mesh,
                        &strip.positions,
                        &strip.colours,
                        &strip.uv,
                        strip.prim & 0x08 == 0,
                    );
                }
                Ok(mesh)
            })
            .collect()
    }

    /// Stand on `cell` displaced by `offset` (world x/z, a walk's +0x34 /
    /// +0x36): the object position and matrix follow.
    pub fn set_place(&mut self, cell: (u8, u8), offset: [f32; 2]) {
        let position = [
            (f32::from(cell.0) - 3.0) * 50.0 + offset[0],
            self.object.position[1],
            (f32::from(cell.1) - 3.0) * 50.0 + offset[1],
        ];
        if position != self.object.position {
            self.object.position = position;
            self.world = object_matrix(&self.loaded.model, &self.object, &self.loaded.sine);
        }
    }

    /// Yaw offset from the side's facing (unit record +0x30): the object's
    /// Y rotation becomes base + turn.
    pub fn set_turn(&mut self, turn: i32) {
        let base = self.base_yaw;
        let yaw = (base + turn) & 0xFFFF;
        if self.object.rotation[1] != yaw {
            self.object.rotation[1] = yaw;
            self.world = object_matrix(&self.loaded.model, &self.object, &self.loaded.sine);
        }
    }

    /// Stand at a world `position` with Y rotation `yaw` (the battle
    /// arena's re-pose, 0x1E64E0). Arena objects are not recentred
    /// (obj+0x1FF = 0, so no grow either) [emu battle-pine 032 / 044 / 060:
    /// both units; their node results match only with k = 0].
    pub fn set_pose(&mut self, position: [f32; 3], yaw: i32) {
        self.object.position = position;
        self.object.recentre = 0;
        self.object.rotation[1] = yaw & 0xFFFF;
        self.base_yaw = yaw & 0xFFFF;
        self.world = object_matrix(&self.loaded.model, &self.object, &self.loaded.sine);
    }

    /// Light the model with an arena terrain's light ([`crate::duel_arena::ArenaLight`]).
    pub fn set_arena_light(&mut self, light: &crate::duel_arena::ArenaLight) {
        let r = self.loaded.sine.rotation(light.angles);
        self.light = roses_client::monster_pose::UnitLight {
            direction: [r[2][0], r[2][1], r[2][2]],
            colour: light.colour.map(f64::from),
            diffuse_scale: f64::from(light.diffuse),
            ambient_scale: f64::from(light.ambient),
        };
    }

    /// SetMotion 0x1F2A40: play `motion` from its start; returns false (and
    /// keeps the current motion) when the model has no such motion.
    pub fn set_motion(&mut self, motion: u8) -> bool {
        let Some(m) = self.loaded.model.motion(motion) else {
            return false;
        };
        self.motion = motion;
        self.clock = MotionClock::new(motion, m.length);
        self.blend = None;
        true
    }

    /// Uniform object scale (the appear controller's a / 128, 0x1F37E0).
    pub fn set_scale(&mut self, scale: f32) {
        if self.object.scale != [scale; 3] {
            self.object.scale = [scale; 3];
            self.world = object_matrix(&self.loaded.model, &self.object, &self.loaded.sine);
        }
    }

    /// Object scale per axis (obj+0x30, written by attack effects).
    pub fn set_scale3(&mut self, scale: [f32; 3]) {
        if self.object.scale != scale {
            self.object.scale = scale;
            self.world = object_matrix(&self.loaded.model, &self.object, &self.loaded.sine);
        }
    }

    /// One original update (two PAL fields at 25 Hz).
    pub fn tick(&mut self) {
        self.clock.advance(1);
    }
}

/// Parse "c,r" with both values 0..=6.
pub fn parse_cell(text: &str) -> Result<(u8, u8)> {
    let parts: Vec<u8> = text
        .split(',')
        .map(|p| p.trim().parse().map_err(|_| format!("Invalid cell {text}")))
        .collect::<Result<_>>()?;
    match parts[..] {
        [c, r] if c <= 6 && r <= 6 => Ok((c, r)),
        _ => Err("A cell is two numbers 0..=6".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registers(test: u64, alpha: u64, clamp: u64, tex1: u64) -> Vec<[u64; 2]> {
        vec![
            [clamp, 0x08],
            [0, 0x34],
            [0, 0x36],
            [test, 0x47],
            [alpha, 0x42],
            [0, 0x06],
            [tex1, 0x14],
        ]
    }

    #[test]
    fn gs_registers_decode_to_draw_state() {
        // Round-2 card 392: CLAMP 5, TEST 0x7000C, ALPHA 0x44, TEX1 0x60,
        // TEX0 TCC 1 (word 13 0x20100005), PRIM 0x3C.
        let state = draw_state(
            &registers(0x7000C, 0x80_0000_0044, 5, 0x60),
            0x3c,
            Some(0x2010_0005),
        )
        .unwrap();
        assert_eq!(
            state,
            DrawState {
                textured: true,
                clamp_u: true,
                clamp_v: true,
                linear: true,
                alpha_greater_than: None,
                texture_alpha: true,
                blend: None,
            }
        );
        let pass2 =
            draw_state(&registers(0x7000D, 0x48, 5, 0x60), 0x7c, Some(0x2010_0005)).unwrap();
        assert_eq!(
            (pass2.alpha_greater_than, pass2.blend),
            (Some(0), Some(Blend::Additive))
        );
        let untextured = draw_state(&registers(0x7000C, 0x44, 0, 0), 0x2c, None).unwrap();
        assert!(!untextured.textured && !untextured.texture_alpha);
        assert!(draw_state(&registers(0x3000C, 0x44, 5, 0x60), 0x3c, Some(0)).is_err());
        assert!(draw_state(&registers(0x7000C, 0x44, 2, 0x60), 0x3c, Some(0)).is_err());
        assert!(draw_state(&registers(0x7000C, 0x64, 5, 0x60), 0x7c, Some(0)).is_err());
        assert!(draw_state(&registers(0x7000C, 0x44, 5, 0x60), 0x3c, None).is_err());
        // TFX DECAL is not supported.
        assert!(draw_state(&registers(0x7000C, 0x44, 5, 0x60), 0x3c, Some(1 << 3)).is_err());
    }

    #[test]
    fn strips_expand_with_flat_colour_from_the_last_vertex() {
        let positions = [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
        let colours = [
            [10, 0, 0, 128],
            [20, 0, 0, 128],
            [30, 0, 0, 128],
            [40, 0, 0, 128],
        ];
        let uv = [[0.0; 2], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let mut gouraud = FrameMesh::default();
        expand_strip(&mut gouraud, &positions, &colours, &uv, false);
        assert_eq!(gouraud.positions.len(), 6);
        assert_eq!(gouraud.colours[3][0], 20.0);
        let mut flat = FrameMesh::default();
        expand_strip(&mut flat, &positions, &colours, &uv, true);
        assert_eq!(
            flat.colours.iter().map(|c| c[0]).collect::<Vec<_>>(),
            [30.0, 30.0, 30.0, 40.0, 40.0, 40.0]
        );
        assert_eq!(flat.uv[5], [1.0, 1.0]);
    }

    #[test]
    fn draw_order_keys_follow_the_order_whatever_the_geometry() {
        let e = DRAW_ORDER_EXTENT;
        for order in 0..MAX_DRAW_GROUPS - 1 {
            assert!(draw_order_key(order, e) < draw_order_key(order + 1, -e));
        }
        let mesh = |z: f32| FrameMesh {
            positions: vec![[0.0, 0.0, z]],
            ..FrameMesh::default()
        };
        assert!(check_draw_order_extent(&[mesh(e), mesh(-e)]).is_ok());
        assert!(check_draw_order_extent(&[mesh(0.0), mesh(e * 1.01)]).is_err());
        assert!(check_draw_order_extent(&[mesh(f32::NAN)]).is_err());
        assert!(check_draw_order_extent(&vec![mesh(0.0); MAX_DRAW_GROUPS + 1]).is_err());
    }

    #[test]
    fn cells_parse_and_validate() {
        assert_eq!(parse_cell("3,6").unwrap(), (3, 6));
        assert!(parse_cell("7,0").is_err());
        assert!(parse_cell("1").is_err());
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;
    use crate::duel_board::{BoardCamera, gs_screen};

    fn manifest_path() -> std::path::PathBuf {
        std::env::var_os("ROSES_DUEL_MODELS_MANIFEST")
            .expect("ROSES_DUEL_MODELS_MANIFEST")
            .into()
    }

    /// Bevy's RenderMesh AABB centre z (midpoint of the position bounds).
    fn aabb_centre_z(mesh: &FrameMesh) -> f32 {
        let (lo, hi) = mesh
            .positions
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
                (lo.min(p[2]), hi.max(p[2]))
            });
        (lo + hi) / 2.0
    }

    /// Cards decoded from the verbatim archive equal the exported ones:
    /// model, missing textures and every texel.
    #[cfg(feature = "renderer")]
    #[test]
    #[ignore = "Requires a private models import with its archive (ROSES_DUEL_MODELS_MANIFEST)"]
    fn private_archive_cards_equal_the_exported_cards() {
        use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
        let path = manifest_path();
        let manifest: Manifest = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let archive = manifest
            .archive
            .as_ref()
            .expect("an import with MONSTER.MRG");
        let root = path.parent().unwrap().canonicalize().unwrap();
        let mut texels = 0usize;
        for entry in &manifest.models {
            let exported = load_model(&path, entry.card).unwrap();
            let (model, textures, missing) =
                decode_from_archive(&root, archive, entry.card).unwrap();
            assert_eq!(model, exported.model, "card {}", entry.card);
            assert_eq!(
                missing.keys().collect::<Vec<_>>(),
                exported.missing_textures.keys().collect::<Vec<_>>()
            );
            assert_eq!(
                textures.keys().collect::<Vec<_>>(),
                exported.textures.keys().collect::<Vec<_>>()
            );
            for (index, (_, bytes)) in &textures {
                let TextureBytes::Rgba {
                    width,
                    height,
                    pixels,
                } = bytes
                else {
                    panic!("archive textures are raw");
                };
                let TextureBytes::Png(png) = &exported.textures[index].1 else {
                    panic!("exported textures are PNG");
                };
                let image = Image::from_buffer(
                    png,
                    ImageType::Extension("png"),
                    CompressedImageFormats::NONE,
                    false,
                    ImageSampler::Default,
                    bevy::asset::RenderAssetUsages::default(),
                )
                .unwrap();
                assert_eq!(image.width(), *width);
                assert_eq!(image.height(), *height);
                assert_eq!(
                    image.data.as_deref(),
                    Some(pixels.as_slice()),
                    "card {} texture {index}",
                    entry.card
                );
                texels += pixels.len() / 4;
            }
        }
        eprintln!("{} cards, {texels} texels equal", manifest.models.len());
    }

    #[test]
    #[ignore = "Requires the private models import (ROSES_DUEL_MODELS_MANIFEST)"]
    fn private_groups_replay_the_original_strip_order() {
        let path = manifest_path();
        let manifest: Manifest = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(!manifest.models.is_empty());
        for entry in &manifest.models {
            let loaded = load_model(&path, entry.card).unwrap();
            let (groups, _) = draw_groups(&loaded);
            let sequence: Vec<(u8, usize, usize)> = groups
                .iter()
                .flat_map(|g| g.strips.iter().map(|&s| (g.pass, g.draw, s)))
                .collect();
            let pass_rank: Vec<(usize, usize, usize)> = sequence
                .iter()
                .map(|&(pass, draw, strip)| {
                    let rank = loaded.model.passes.iter().position(|p| p.pass == pass);
                    (rank.unwrap(), draw, strip)
                })
                .collect();
            assert!(
                pass_rank.windows(2).all(|w| w[0] < w[1]),
                "card {}: groups out of the original order",
                entry.card
            );
        }
    }

    #[test]
    #[ignore = "Requires the private models import (ROSES_DUEL_MODELS_MANIFEST)"]
    fn private_hoshiningen_blended_pass_sorts_after_the_opaque_pass() {
        // Card 383 at side 1 has blended ribbon groups whose AABB centres
        // lie nearer than its opaque body; the sort keys must ignore that.
        let loaded = load_model(&manifest_path(), 383).unwrap();
        let mut scene = UnitScene::new(loaded, 0, (3, 0), 1, 1.0).unwrap();
        let passes: Vec<u8> = scene.groups.iter().map(|g| g.pass).collect();
        assert!(passes.contains(&1) && passes.windows(2).all(|w| w[0] <= w[1]));
        for _ in 0..60 {
            let frame = scene.frame().unwrap();
            let keys: Vec<f32> = frame
                .iter()
                .enumerate()
                .map(|(order, mesh)| draw_order_key(order, aabb_centre_z(mesh)))
                .collect();
            assert!(keys.windows(2).all(|w| w[0] < w[1]), "{keys:?}");
            scene.tick();
        }
    }

    #[test]
    #[ignore = "Requires the private models import (ROSES_DUEL_MODELS_MANIFEST)"]
    fn private_card_392_stands_on_its_cell_in_the_board_projection() {
        let path =
            std::env::var_os("ROSES_DUEL_MODELS_MANIFEST").expect("ROSES_DUEL_MODELS_MANIFEST");
        let loaded = load_model(Path::new(&path), 392).unwrap();
        let mut scene = UnitScene::new(loaded, 0, (3, 6), 0, 1.0).unwrap();
        assert!(scene.skipped.is_empty(), "{:?}", scene.skipped);
        assert_eq!(scene.groups.len(), 23);
        for _ in 0..10 {
            scene.tick();
        }
        assert_eq!(scene.clock.time, 11.0);
        let frame = scene.frame().unwrap();
        let view = BoardCamera::snapped((3, 6)).view().unwrap();
        let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
        for mesh in &frame {
            for p in &mesh.positions {
                let s = gs_screen(&view, p.map(f64::from));
                for k in 0..2 {
                    min[k] = min[k].min(s[k]);
                    max[k] = max[k].max(s[k]);
                }
            }
        }
        // The round-2 dump draws the leader within x 231..284.4, y 188..363.4.
        assert!(
            min[0] > 230.0 && max[0] < 285.5 && min[1] > 187.0 && max[1] < 364.5,
            "{min:?} {max:?}"
        );
    }
}

/// The facing yaw of an N-player duel's E / W unit (a port addition;
/// never reached without the `online` feature).
fn side_model_yaw(side: u8) -> i32 {
    #[cfg(feature = "online")]
    {
        crate::duel_multi::model_yaw(side)
    }
    #[cfg(not(feature = "online"))]
    {
        let _ = side;
        0
    }
}
