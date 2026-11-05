//! Campaign map scene model for the optional renderer (no GPU code here).
//!
//! Input is the private output of `roses import-campaign-map` (manifest,
//! map-data.json, raw RH0 geometry JSON, GS-byte PNG textures, message
//! images), hash-checked on load. The frame logic is
//! `roses_client::campaign_map::MapModel`; this module converts the models,
//! places them and projects them with the traced map camera. Sources (PAL EE
//! addresses, private research `story/the project notes):
//!
//! - camera eye = target + (0, 112, 112) looking at the target on y = 0, GS
//!   projection x = 256 + 800·x/z, y = 256 + 1074.2855·y/z (as the board);
//! - the six tiles (slots 34..=39) and the two sea pieces (40, 41) are drawn
//!   at the world origin: their object positions in the map savestate equal
//!   their bounding-box centres [O]; node bases (slot 34 + base model) and
//!   castles (34 + castle model) at the node position, hidden nodes sunk by
//!   12 (18 for node 16), rising nodes shake ±0.4 in x [F]; red dots (slot
//!   62) at the spline samples;
//! - per-vertex colour = floor(min(max, a·g·ambient + d·g·diffuse·max(0,
//!   n·L)·light)) with the object scales (d, a, g) = (0.8, 0.8, 1.0) of the
//!   map objects [O: object +0x120 in the map savestate] and the board unit
//!   light direction as an explicit stand-in [H: the map light set is not
//!   traced]; strips with PRIM ABE blend with the draw's ALPHA register
//!   (0x44 standard, 0x48 additive).
//!
//! The ship is drawn from the model's 0x22CA00 position/heading and the
//! letterbox from 0x233750's counters.
//!
//! Not modelled: the leader figure (MONSTER data; a plain placeholder marker),
//! the place caption's own layout (placeholder position), the sea pieces'
//! motion records, the white route dots, HUD/panel/cursor art (message images
//! and a crosshair at explicit placeholder positions).
use crate::camera::{self, Mat4};
use crate::duel_board::{self, strip_indices};
use crate::{Result, bounded_read, hash_valid, validate_relative_path};
use roses_client::campaign_map::{self as map, MapData, MapModel as MapSim, NODE_COUNT};
use roses_client::monster_pose::{UnitLight, vertex_colour};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MANIFEST_SCHEMA: &str = map::SCHEMA;
pub const TILE_SLOTS: [u16; 6] = [34, 35, 36, 37, 38, 39];
pub const PIECE_SLOTS: [u16; 2] = [40, 41];
pub const SHIP_SLOT: u16 = 61;
pub const RED_DOT_SLOT: u16 = 62;
/// The White Rose side's path dot (0x22C330 with side 1) and the second
/// boat (model 30, ship kinds 2 and 3) [F MAP.md §1].
pub const WHITE_DOT_SLOT: u16 = 63;
pub const MODEL_SLOT_SHIP_BASE: i16 = 34;
/// Model index + this = object slot (node table +0x10/+0x12).
pub const MODEL_SLOT_BASE: i16 = 34;
pub const HIDDEN_SINK: f64 = 12.0;
pub const HIDDEN_SINK_NODE_16: f64 = 18.0;
pub const RISE_SHAKE: f64 = 0.4;
/// Map object light: scales from object +0x120 (0.8, 0.8, 1.0) [O], the
/// direction and colour of the board unit light [H].
pub const MAP_LIGHT: UnitLight = UnitLight {
    direction: [-0.707_106_769_084_930_4, 0.707_106_769_084_930_4, 0.0],
    colour: [0.5; 3],
    diffuse_scale: 0.800_000_011_920_929,
    ambient_scale: 0.800_000_011_920_929,
};
pub const MAP_BRIGHTNESS: f64 = 1.0;
/// Placeholder layout of the 512-pixel raw canvas (untraced window art).
pub const CAPTION_POSITION: (u32, u32) = (82, 40);
/// Placeholder position of the voyage's place caption (style 1 layout of
/// 0x2314B8 not traced).
pub const VOYAGE_CAPTION_POSITION: (u32, u32) = (82, 80);
pub const SPEAKER_POSITION: (u32, u32) = (64, 372);
pub const PAGE_POSITION: (u32, u32) = (64, 400);
pub const ROSE_TEXT_POSITION: (u32, u32) = (118, 240);
/// The over-DC message's text row (0x236380: the window at y 288, its
/// text 4 below; x centres the window of width text + 16 on 256).
pub const OVER_DC_TEXT_Y: u32 = 292;
pub const CURSOR_HALF: u32 = 5;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_GEOMETRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TEXTURE_BYTES: u64 = 4 * 1024 * 1024;

// ---------------------------------------------------------------- manifest
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub source: ManifestSource,
    pub map_data: FileRef,
    pub geometry: FileRef,
    pub models: Vec<ModelRef>,
    #[serde(default)]
    pub text_images: BTreeMap<String, TextImageRef>,
    /// DATA/TEXANM.MRG (talking faces), absent in older imports.
    #[serde(default)]
    pub texanm: Option<FileRef>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ManifestSource {
    pub iso_sha256: String,
    pub executable_sha256: String,
    pub model_archive: ArchiveRef,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ArchiveRef {
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct FileRef {
    pub path: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ModelRef {
    pub slot: u16,
    pub record_sha256: String,
    pub textures: Vec<TextureRef>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct TextureRef {
    #[serde(default)]
    pub index: usize,
    pub path: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Deserialize)]
pub struct TextImageRef {
    pub path: Option<String>,
    pub sha256: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub not_rendered: Option<String>,
}

// ---------------------------------------------------------------- geometry
#[derive(Clone, Debug, Deserialize)]
struct Geometry {
    schema_version: u32,
    archive_sha256: String,
    models: Vec<GeoModel>,
}
#[derive(Clone, Debug, Deserialize)]
struct GeoModel {
    slot: u16,
    source_sha256: String,
    node_zero_scale: [f32; 3],
    node_zero_rotation_words: [u32; 3],
    /// RH0 node records (16 words each), node 0 first.
    raw_node_words: Vec<[u32; 16]>,
    textures: Vec<GeoTexture>,
    draws: Vec<GeoDraw>,
    strips: Vec<GeoStrip>,
}
#[derive(Clone, Debug, Deserialize)]
struct GeoTexture {
    raw_header_words: Vec<u32>,
}
#[derive(Clone, Debug, Deserialize)]
struct GeoDraw {
    alpha_chain: bool,
    texture_index: Option<u16>,
    material_words: [u32; 12],
    gs_registers: Vec<[u64; 2]>,
}
#[derive(Clone, Debug, Deserialize)]
struct GeoStrip {
    draw_index: u16,
    textured: bool,
    blended: bool,
    vertices: Vec<GeoVertex>,
}
#[derive(Clone, Debug, Deserialize)]
struct GeoVertex {
    texture_raw: [i16; 3],
    normal_raw: [i16; 3],
    position_raw: [i16; 3],
}

/// GS ALPHA register modes used by the map draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// PRIM ABE off.
    Opaque,
    /// ALPHA 0x44: (Cs − Cd)·As + Cd.
    Standard,
    /// ALPHA 0x48: Cs·As + Cd.
    Additive,
}

/// One GPU draw: a draw group's strips with one texture/blend mode.
#[derive(Clone, Debug)]
pub struct DrawMesh {
    pub draw_index: usize,
    pub alpha_chain: bool,
    pub textured: bool,
    pub blend: Blend,
    pub positions_raw: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    /// Per-vertex colour in GS units (RGB 0..255, A with 128 = 1.0).
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    /// Manifest texture path (hash-checked PNG in `LoadedMap::textures`).
    pub texture: Option<String>,
    pub clamp_u: bool,
    pub clamp_v: bool,
    pub linear: bool,
    /// ATST GREATER reference when alpha testing is on (AFAIL KEEP).
    pub alpha_greater_than: Option<u8>,
    /// TEX0 TCC: texture alpha used (RGBA) or vertex alpha only (RGB).
    pub texture_alpha: bool,
}
#[derive(Clone, Debug)]
pub struct MapModel {
    pub slot: u16,
    pub scale: [f64; 3],
    pub node_rotation_y: u16,
    /// Node 0's translation (RH0 +0x20) when its no-translation flag (bit 0
    /// of +0x0C) is clear: only the ship (0, −7.1, 0) of the map's
    /// single-node models (node evaluation as `monster_pose`, 0x210050).
    pub node_translation: [f64; 3],
    pub draws: Vec<DrawMesh>,
}

#[derive(Debug)]
pub struct LoadedMap {
    pub manifest: Manifest,
    pub manifest_sha256: String,
    pub data: MapData,
    pub models: BTreeMap<u16, MapModel>,
    /// Hash-checked PNG bytes by manifest path; kept in memory.
    pub textures: BTreeMap<String, Vec<u8>>,
    /// Hash-checked message image PNGs by message id.
    pub text_images: BTreeMap<u16, (Vec<u8>, u32, u32)>,
    /// Hash-checked TEXANM.MRG bytes, when the import has them.
    pub texanm: Option<Vec<u8>>,
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
        return Err("Map file resolves outside the import directory".into());
    }
    Ok(path)
}
fn register(draw: &GeoDraw, address: u64) -> Result<u64> {
    draw.gs_registers
        .iter()
        .find(|pair| pair[1] == address)
        .map(|pair| pair[0])
        .ok_or_else(|| format!("GS register {address:#x} missing"))
}
fn normal(raw: [i16; 3]) -> [f64; 3] {
    let v = raw.map(f64::from);
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n < 1e-9 {
        [0.0, 1.0, 0.0]
    } else {
        v.map(|x| x / n)
    }
}

fn convert_model(model: &GeoModel, textures: &[TextureRef], slot: u16, light: &UnitLight) -> Result<MapModel> {
    let id = format!("slot {slot}");
    let mut draws = Vec::new();
    for (index, draw) in model.draws.iter().enumerate() {
        let material: [f32; 12] = draw.material_words.map(f32::from_bits);
        if material.iter().any(|v| !v.is_finite()) {
            return Err(format!("{id}: invalid material"));
        }
        let (mut clamp_u, mut clamp_v, mut linear, mut alpha_greater_than, mut blend) =
            (false, false, true, None, Blend::Opaque);
        let mut texture_alpha = false;
        let mut path = None;
        if let Some(texture_index) = draw.texture_index {
            let clamp = register(draw, 0x08)?;
            let test = register(draw, 0x47)?;
            let tex1 = register(draw, 0x14)?;
            let alpha = register(draw, 0x42)?;
            let (wms, wmt) = (clamp & 3, (clamp >> 2) & 3);
            if wms > 1 || wmt > 1 {
                return Err(format!("{id}: region clamp modes are not supported"));
            }
            let (mmag, mmin) = ((tex1 >> 5) & 1, (tex1 >> 6) & 7);
            if mmag != mmin || mmin > 1 {
                return Err(format!(
                    "{id}: mipmapped or mixed texture filters are not supported"
                ));
            }
            let (ate, atst, aref, afail) = (
                test & 1,
                (test >> 1) & 7,
                (test >> 4) & 255,
                (test >> 12) & 3,
            );
            let (zte, ztst) = ((test >> 16) & 1, (test >> 17) & 3);
            if (ate == 1 && (atst != 6 || afail != 0))
                || zte != 1
                || ztst != 3
                || test & 0xc000 != 0
            {
                return Err(format!("{id}: unsupported TEST register {test:#x}"));
            }
            blend = match alpha & 0xFF {
                0x44 => Blend::Standard,
                0x48 => Blend::Additive,
                other => return Err(format!("{id}: unsupported ALPHA register {other:#x}")),
            };
            let texture = model
                .textures
                .get(usize::from(texture_index))
                .ok_or_else(|| format!("{id}: draw texture index out of range"))?;
            let tex0_high = *texture
                .raw_header_words
                .get(13)
                .ok_or_else(|| format!("{id}: texture header truncated"))?;
            if (tex0_high >> 3) & 3 != 0 {
                return Err(format!("{id}: only TFX MODULATE is supported"));
            }
            clamp_u = wms == 1;
            clamp_v = wmt == 1;
            linear = mmag == 1;
            alpha_greater_than = (ate == 1).then_some(aref as u8);
            texture_alpha = (tex0_high >> 2) & 1 == 1;
            path = Some(
                textures
                    .iter()
                    .find(|t| t.index == usize::from(texture_index))
                    .ok_or_else(|| format!("{id}: manifest lacks texture {texture_index}"))?
                    .path
                    .clone(),
            );
        }
        // An untextured strip may sit in a textured draw (battle stages):
        // its own mesh, without the texture.
        for (blended, textured) in [(false, true), (false, false), (true, true), (true, false)] {
            if textured && path.is_none() {
                continue;
            }
            let mut mesh = DrawMesh {
                draw_index: index,
                alpha_chain: draw.alpha_chain,
                textured,
                blend: if blended { blend } else { Blend::Opaque },
                positions_raw: Vec::new(),
                uv: Vec::new(),
                colors: Vec::new(),
                indices: Vec::new(),
                texture: if textured { path.clone() } else { None },
                clamp_u,
                clamp_v,
                linear,
                alpha_greater_than,
                texture_alpha,
            };
            for strip in model
                .strips
                .iter()
                .filter(|s| {
                    usize::from(s.draw_index) == index
                        && s.blended == blended
                        && s.textured == textured
                })
            {
                let base = mesh.positions_raw.len() as u32;
                mesh.indices
                    .extend(strip_indices(base, strip.vertices.len() as u32));
                for v in &strip.vertices {
                    mesh.positions_raw.push(v.position_raw.map(f32::from));
                    mesh.uv.push([
                        f32::from(v.texture_raw[0]) / 4096.0,
                        f32::from(v.texture_raw[1]) / 4096.0,
                    ]);
                    let c =
                        vertex_colour(&material, normal(v.normal_raw), light, MAP_BRIGHTNESS);
                    mesh.colors.push(c.map(f32::from));
                }
            }
            if !mesh.indices.is_empty() {
                draws.push(mesh);
            }
        }
    }
    let scale = model.node_zero_scale.map(f64::from);
    if scale.iter().any(|s| !s.is_finite() || *s <= 0.0) || draws.is_empty() {
        return Err(format!("{id}: invalid model scale or no draws"));
    }
    Ok(MapModel {
        slot,
        scale,
        node_rotation_y: model.node_zero_rotation_words[1] as u16,
        node_translation: match model.raw_node_words.first() {
            Some(words) if words[3] & 1 == 0 => {
                std::array::from_fn(|i| f64::from(f32::from_bits(words[8 + i])))
            }
            _ => [0.0; 3],
        },
        draws,
    })
}

/// Load and hash-check a campaign map import. Every referenced file must lie
/// inside the manifest's directory.
pub fn load_map(manifest_path: &Path) -> Result<LoadedMap> {
    let bytes = bounded_read(manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Map manifest: {e}"))?;
    if manifest.schema != MANIFEST_SCHEMA
        || ![
            &manifest.source.iso_sha256,
            &manifest.source.executable_sha256,
            &manifest.source.model_archive.sha256,
            &manifest.geometry.sha256,
            &manifest.map_data.sha256,
        ]
        .into_iter()
        .all(|h| hash_valid(h))
    {
        return Err("Unsupported map manifest schema or fingerprints".into());
    }
    let root = manifest_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let data_bytes = bounded_read(&child(&root, &manifest.map_data.path)?, MAX_MANIFEST_BYTES)?;
    if sha(&data_bytes) != manifest.map_data.sha256 {
        return Err("Map data fingerprint mismatch".into());
    }
    let data: MapData =
        serde_json::from_slice(&data_bytes).map_err(|e| format!("map-data.json: {e}"))?;
    data.validate()?;
    let geometry_bytes = bounded_read(&child(&root, &manifest.geometry.path)?, MAX_GEOMETRY_BYTES)?;
    if sha(&geometry_bytes) != manifest.geometry.sha256 {
        return Err("Map geometry fingerprint mismatch".into());
    }
    let geometry: Geometry =
        serde_json::from_slice(&geometry_bytes).map_err(|e| format!("Map geometry: {e}"))?;
    if geometry.schema_version != 1
        || geometry.archive_sha256 != manifest.source.model_archive.sha256
    {
        return Err("Map geometry does not match the manifest source".into());
    }
    let refs: BTreeMap<u16, &ModelRef> = manifest.models.iter().map(|m| (m.slot, m)).collect();
    let mut models = BTreeMap::new();
    let mut textures = BTreeMap::new();
    let mut decoded = 0u64;
    for model in &geometry.models {
        let reference = refs
            .get(&model.slot)
            .ok_or_else(|| format!("Manifest lacks model slot {}", model.slot))?;
        if reference.record_sha256 != model.source_sha256 {
            return Err(format!("slot {}: record fingerprint mismatch", model.slot));
        }
        for texture in &reference.textures {
            if textures.contains_key(&texture.path) {
                continue;
            }
            let png = bounded_read(&child(&root, &texture.path)?, MAX_TEXTURE_BYTES)?;
            if !hash_valid(&texture.sha256) || sha(&png) != texture.sha256 {
                return Err(format!("Texture fingerprint mismatch: {}", texture.path));
            }
            if png.get(..8) != Some(b"\x89PNG\r\n\x1a\n")
                || texture.width == 0
                || texture.height == 0
                || texture.width > 1024
                || texture.height > 1024
            {
                return Err(format!("Texture is not a bounded PNG: {}", texture.path));
            }
            decoded += u64::from(texture.width * texture.height) * 4;
            if decoded > 64 * 1024 * 1024 {
                return Err("Map texture budget exceeded".into());
            }
            textures.insert(texture.path.clone(), png);
        }
        models.insert(
            model.slot,
            convert_model(model, &reference.textures, model.slot, &MAP_LIGHT)?,
        );
    }
    for slot in TILE_SLOTS.iter().chain(&PIECE_SLOTS).chain([&RED_DOT_SLOT]) {
        if !models.contains_key(slot) {
            return Err(format!("Map import lacks model slot {slot}"));
        }
    }
    let mut text_images = BTreeMap::new();
    for (id, image) in &manifest.text_images {
        let (Some(path), Some(hash), Some(width), Some(height)) =
            (&image.path, &image.sha256, image.width, image.height)
        else {
            continue;
        };
        let id: u16 = id
            .parse()
            .map_err(|_| "Text image id is not a message id")?;
        let png = bounded_read(&child(&root, path)?, MAX_TEXTURE_BYTES)?;
        if !hash_valid(hash)
            || sha(&png) != *hash
            || width == 0
            || height == 0
            || width > 512
            || height > 128
        {
            return Err(format!("Text image mismatch: {path}"));
        }
        text_images.insert(id, (png, width, height));
    }
    let texanm = match &manifest.texanm {
        Some(file) => {
            let archive = bounded_read(&child(&root, &file.path)?, 16 * 1024 * 1024)?;
            if !hash_valid(&file.sha256) || sha(&archive) != file.sha256 {
                return Err("TEXANM.MRG fingerprint mismatch".into());
            }
            Some(archive)
        }
        None => None,
    };
    Ok(LoadedMap {
        manifest,
        manifest_sha256: sha(&bytes),
        data,
        models,
        textures,
        text_images,
        texanm,
    })
}

/// Models of any geometry.json written with the campaign map's grammar
/// (`decode_pal_model_bank`, e.g. the battle arena import), converted like
/// the map's, with each slot's manifest textures and the light `light_of`
/// gives the slot. Checks the archive fingerprint and each record's against
/// `textures_by_slot`.
pub(crate) fn models_from_geometry(
    geometry_bytes: &[u8],
    archive_sha256: &str,
    textures_by_slot: &BTreeMap<u16, (String, Vec<TextureRef>)>,
    light_of: &dyn Fn(u16) -> UnitLight,
) -> Result<Vec<MapModel>> {
    let geometry: Geometry =
        serde_json::from_slice(geometry_bytes).map_err(|e| format!("geometry: {e}"))?;
    if geometry.schema_version != 1 || geometry.archive_sha256 != archive_sha256 {
        return Err("geometry does not match its manifest source".into());
    }
    let mut out = Vec::new();
    for model in &geometry.models {
        let Some((record_sha256, textures)) = textures_by_slot.get(&model.slot) else {
            return Err(format!("manifest lacks model slot {}", model.slot));
        };
        if *record_sha256 != model.source_sha256 {
            return Err(format!("slot {}: record fingerprint mismatch", model.slot));
        }
        out.push(convert_model(model, textures, model.slot, &light_of(model.slot))?);
    }
    Ok(out)
}

// ---------------------------------------------------------------- placement
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub slot: u16,
    pub label: String,
    pub world: Mat4,
    /// Extra alpha factor 0..=1 (dot fade-in); 1 for everything else.
    pub alpha: f32,
}

#[cfg(test)]
fn placed(model: &MapModel, position: [f64; 3]) -> Mat4 {
    placed_rotated(model, position, model.node_rotation_y as i16)
}

/// Placement with an explicit y rotation (65536 per turn, as 0x1EF6F0):
/// object translation and rotation, node 0's translation, node scale.
pub(crate) fn placed_rotated(model: &MapModel, position: [f64; 3], rotation_y: i16) -> Mat4 {
    let rotation = camera::rotation_y(rotation_y);
    let object = camera::mul(&duel_board::translation(position), &rotation);
    camera::mul(
        &camera::mul(&object, &duel_board::translation(model.node_translation)),
        &duel_board::scale(model.scale),
    )
}

/// Vertical offset of a node's base and castle from its state.
pub fn node_sink(sim: &MapSim, node: u8) -> f64 {
    let sink = if node == 16 {
        HIDDEN_SINK_NODE_16
    } else {
        HIDDEN_SINK
    };
    let rise = &sim.rises[usize::from(node)];
    if rise.rising {
        -sink * (1.0 - f64::from(rise.level) / f64::from(map::RISE_FULL))
    } else if sim.save.state(node) == map::STATE_HIDDEN {
        -sink
    } else {
        0.0
    }
}

/// Instances in draw order: tiles, ship, node bases and castles, red dots,
/// then the sea pieces (blended last).
pub fn instances(sim: &MapSim, loaded: &LoadedMap) -> Vec<Instance> {
    let mut out = Vec::new();
    let mut push =
        |slot: u16, label: String, position: [f64; 3], alpha: f32, rotation: Option<i16>| {
            if let Some(model) = loaded.models.get(&slot) {
                let rotation = rotation.unwrap_or(model.node_rotation_y as i16);
                out.push(Instance {
                    slot,
                    label,
                    world: placed_rotated(model, position, rotation),
                    alpha,
                });
            }
        };
    for slot in TILE_SLOTS {
        push(slot, format!("tile {slot}"), [0.0; 3], 1.0, None);
    }
    // The ship (0x22CA00): the model's position and heading, slot 34 +
    // model (27 the red ship, 30 Mako's boat for kinds 2 and 3).
    if sim.ship.visible {
        let p = sim.ship.position;
        push(
            (MODEL_SLOT_SHIP_BASE + sim.ship_model()) as u16,
            "ship".into(),
            [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])],
            1.0,
            Some(sim.ship.heading as i16),
        );
    }
    // 0x22A5C8..0x22A618: Stonehenge's base and castle are drawn at its
    // position every frame before the node loop, whatever its state (the
    // loop draws it again, sunk while hidden) [F; emu 588-x6 screenshot].
    let stonehenge = sim.data.node(map::STONEHENGE_NODE);
    for (model, part) in [
        (stonehenge.base_model, "base"),
        (stonehenge.castle_model, "castle"),
    ] {
        push(
            (model + MODEL_SLOT_BASE) as u16,
            format!("stonehenge {part} (always)"),
            stonehenge.position.map(f64::from),
            1.0,
            None,
        );
    }
    for node in &sim.data.nodes {
        let y = node_sink(sim, node.index);
        let shake = if sim.rises[usize::from(node.index)].rising && sim.frame % 2 == 0 {
            RISE_SHAKE
        } else if sim.rises[usize::from(node.index)].rising {
            -RISE_SHAKE
        } else {
            0.0
        };
        let position = [
            f64::from(node.position[0]) + shake,
            f64::from(node.position[1]) + y,
            f64::from(node.position[2]),
        ];
        let base = (node.base_model + MODEL_SLOT_BASE) as u16;
        let castle = (node.castle_model + MODEL_SLOT_BASE) as u16;
        push(
            base,
            format!("node {} base", node.name),
            position,
            1.0,
            None,
        );
        push(
            castle,
            format!("node {} castle", node.name),
            position,
            1.0,
            None,
        );
    }
    let (side, dot_slot) = if sim.save.side == 0 {
        (0, RED_DOT_SLOT)
    } else {
        (1, WHITE_DOT_SLOT)
    };
    for (index, state) in sim.paths.iter().enumerate().take(map::PATHS_PER_SIDE) {
        if state.state == 0 {
            continue;
        }
        let path = sim.data.path(side, index as u8);
        if path.sea_route {
            continue;
        }
        for (k, dot) in path.dots.iter().enumerate() {
            let alpha = state.alpha.get(k).copied().unwrap_or(0);
            if alpha <= 0 {
                continue;
            }
            push(
                dot_slot,
                format!("path {index} dot {k}"),
                [f64::from(dot[0]), 0.0, f64::from(dot[1])],
                (f64::from(alpha) / f64::from(map::DOT_FULL_ALPHA)).min(1.0) as f32,
                None,
            );
        }
    }
    for slot in PIECE_SLOTS {
        push(slot, format!("sea piece {slot}"), [0.0; 3], 1.0, None);
    }
    out
}

/// Whether the leader placeholder is shown: not during the voyages, whose
/// screenshot (588-x6) shows no figure (the figure's drawing is untraced).
pub fn figure_visible(sim: &MapSim) -> bool {
    !matches!(sim.handler, map::Handler::Sail { .. })
}

/// Placeholder marker position for the leader figure (not the original model).
pub fn figure_marker(sim: &MapSim) -> [f64; 3] {
    [sim.figure.position[0], 0.0, sim.figure.position[1]]
}

/// Put the leader figure's model in this frame's object state: both of
/// object 30's rotations (obj+0x10 from the map's init, then obj+0x20) as
/// the unit draw 0x1EF8F0 applies them, the model's own grow (obj+0x1E8).
pub fn place_figure(scene: &mut crate::duel_models::UnitScene, object: &map::FigureObject) {
    scene.object = object.unit_object(scene.object.grow);
    scene.world = roses_client::monster_pose::object_matrix(
        &scene.loaded.model,
        &scene.object,
        &scene.loaded.sine,
    );
}

// ---------------------------------------------------------------- camera
pub fn view(sim: &MapSim) -> Option<Mat4> {
    let target = [sim.camera_target[0], 0.0, sim.camera_target[1]];
    camera::look_at(map::camera_eye(sim.camera_target), target)
}
/// Backend clip matrix (see `duel_board::ndc_from_source`).
pub fn ndc(view: &Mat4, world: &Mat4) -> Mat4 {
    duel_board::ndc_from_source(view, world)
}

// ---------------------------------------------------------------- 2D layer
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerItem {
    /// A message image at a raw-canvas position, limited to its first `lines`.
    Text {
        message: u16,
        x: u32,
        y: u32,
        lines: Option<u32>,
    },
    /// A message image centred on x 256 (its width is the image's).
    CenteredText { message: u16, y: u32 },
    /// The hover panel's place tooltip at the cursor (0x2361B8..0x236268):
    /// window width w = text + 16, x = cursor x − w/2 clamped to 120 and
    /// then to 370 − w, y = cursor y + 32 clamped to 128..=354; the text
    /// at (x + 8, y + 4). The window art is untraced (text only).
    Tooltip { message: u16, cursor: (i32, i32) },
    /// The crosshair placeholder at the cursor.
    Cursor { x: i32, y: i32 },
    /// A place caption (style 1) at a placeholder position with an alpha
    /// 0..=255 from the original's four caption alphas (their mean: which
    /// part each one fades is not traced).
    Caption {
        message: u16,
        x: u32,
        y: u32,
        alpha: u8,
    },
    /// The letterbox (0x233750): opaque black bars over rows 0..height and
    /// 512 − height..512 of the raw canvas.
    Letterbox { height: u32 },
}

/// What the placeholder layer shows this frame (message images only).
pub fn layer_items(sim: &MapSim) -> Vec<LayerItem> {
    let mut items = Vec::new();
    if let Some(text) = &sim.text {
        if text.phase >= 2 && text.openness > 0 {
            // The style-2 window names its own speaker (W+53BE, event
            // 111's Seto); the dialogue window the hovered node's.
            let speaker = text.speaker.or_else(|| {
                sim.hover_text(sim.save.hovered)
                    .and_then(|t| t.speaker_message)
            });
            if let Some(speaker) = speaker {
                items.push(LayerItem::Text {
                    message: speaker,
                    x: SPEAKER_POSITION.0,
                    y: SPEAKER_POSITION.1,
                    lines: None,
                });
            }
            if text.phase >= 3 && text.phase < 11 {
                items.push(LayerItem::Text {
                    message: text.message,
                    x: PAGE_POSITION.0,
                    y: PAGE_POSITION.1,
                    lines: (text.phase == 3)
                        .then(|| text.revealed / map::REVEAL_FRAMES_PER_LINE + 1),
                });
            }
        }
    } else if sim.rose.is_some() {
        items.push(LayerItem::Text {
            message: map::ROSE_CARD_MESSAGE,
            x: ROSE_TEXT_POSITION.0,
            y: ROSE_TEXT_POSITION.1,
            lines: None,
        });
    } else if sim.over_dc {
        // W+5448: MSG 250 "Exceeded DC Level!" replaces the place tooltip
        // while Cross is held (0x236170). This placeholder (text only) is
        // used without the HUD; the HUD draws the original's window
        // (`campaign_map_hud::tooltip`).
        items.push(LayerItem::CenteredText {
            message: map::OVER_DC_MESSAGE,
            y: OVER_DC_TEXT_Y,
        });
    } else if matches!(sim.handler, map::Handler::Free) && sim.travel == 0 {
        items.push(LayerItem::Text {
            message: map::CAPTION_MESSAGE,
            x: CAPTION_POSITION.0,
            y: CAPTION_POSITION.1,
            lines: None,
        });
    }
    if let Some(message) = sim.place_tooltip() {
        items.push(LayerItem::Tooltip {
            message,
            cursor: (i32::from(sim.cursor.0), i32::from(sim.cursor.1)),
        });
    }
    if let Some(caption) = &sim.caption {
        let mean = caption.alpha.iter().sum::<i32>() / 4;
        let alpha = (mean * 255 / 0x80).clamp(0, 255) as u8;
        if alpha > 0 {
            items.push(LayerItem::Caption {
                message: caption.message,
                x: VOYAGE_CAPTION_POSITION.0,
                y: VOYAGE_CAPTION_POSITION.1,
                alpha,
            });
        }
    }
    let height = sim.letterbox.height();
    if height > 0 {
        items.push(LayerItem::Letterbox {
            height: height as u32,
        });
    }
    // The story window's text (11120/11124/11149) without the HUD: centred
    // as the window's (x + 6 = 256 - text width / 2), its frame not drawn.
    if let Some(window) = &sim.story_window {
        items.push(LayerItem::CenteredText {
            message: window.message,
            y: map::STORY_WINDOW_TEXT_Y as u32,
        });
    }
    if matches!(sim.handler, map::Handler::Free) && sim.fade >= map::FADE_FULL {
        items.push(LayerItem::Cursor {
            x: i32::from(sim.cursor.0),
            y: i32::from(sim.cursor.1),
        });
    }
    items
}

/// Parse `--replay`: one pad per 25 Hz update (`up`, `down`, `left`,
/// `right`, `cross`, `none`, `a+b`, `x*n`).
pub fn parse_replay(text: &str) -> Result<Vec<map::Pad>> {
    let mut out = Vec::new();
    for entry in text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (spec, count) = match entry.split_once('*') {
            Some((spec, n)) => (
                spec,
                n.parse::<usize>()
                    .map_err(|_| format!("Invalid replay repeat {entry:?}"))?,
            ),
            None => (entry, 1),
        };
        let mut pad = map::Pad::NONE;
        for button in spec.split('+') {
            match button {
                "up" => pad.up = true,
                "down" => pad.down = true,
                "left" => pad.left = true,
                "right" => pad.right = true,
                "cross" => pad.cross = true,
                "select" => pad.menu.select = true,
                "circle" => pad.menu.circle = true,
                "square" => pad.menu.square = true,
                "l1" => pad.menu.l1 = true,
                "r1" => pad.menu.r1 = true,
                "none" => {}
                other => return Err(format!("Unknown replay button {other:?}")),
            }
        }
        if out.len() + count > 4096 {
            return Err("--replay accepts at most 4096 updates".into());
        }
        out.extend(std::iter::repeat_n(pad, count));
    }
    Ok(out)
}

/// The hovered node's anchor in the raw canvas for reports.
pub fn anchor_report(sim: &MapSim) -> Vec<(u8, i32, i32)> {
    (0..NODE_COUNT)
        .filter_map(|n| sim.anchors[n].map(|(p, _)| (n as u8, p.0, p.1)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_parses_held_and_pressed_buttons() {
        let pads = parse_replay("up*2, cross, up+left, none").unwrap();
        assert_eq!(pads.len(), 5);
        assert!(pads[0].up && !pads[0].cross);
        assert!(pads[2].cross);
        assert!(pads[3].up && pads[3].left);
        assert_eq!(pads[4], map::Pad::NONE);
        assert!(parse_replay("jump").is_err());
        assert!(parse_replay("up*5000").is_err());
    }

    #[test]
    fn placement_scales_and_sinks_hidden_nodes() {
        let model = MapModel {
            slot: 43,
            scale: [0.5; 3],
            node_rotation_y: 0,
            node_translation: [0.0; 3],
            draws: Vec::new(),
        };
        let m = placed(&model, [1.0, -12.0, 3.0]);
        assert_eq!(m[3], [1.0, -12.0, 3.0, 1.0]);
        assert_eq!(m[0][0], 0.5);
        assert_eq!(normal([0, 0, 0]), [0.0, 1.0, 0.0]);
        assert_eq!(normal([0, 3000, 0]), [0.0, 1.0, 0.0]);
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;

    #[test]
    #[ignore = "Requires the private campaign-map import (ROSES_CAMPAIGN_MAP_MANIFEST)"]
    fn private_import_loads_and_projects_the_first_map_frame() {
        let path =
            std::env::var_os("ROSES_CAMPAIGN_MAP_MANIFEST").expect("ROSES_CAMPAIGN_MAP_MANIFEST");
        let loaded = load_map(Path::new(&path)).unwrap();
        assert_eq!(loaded.models.len(), 31);
        let sim = MapSim::at_event_zero(loaded.data.clone()).unwrap();
        let list = instances(&sim, &loaded);
        // 6 tiles, ship, Stonehenge's fixed pair, 22 bases + 22 castles,
        // dots of paths 1 (13) and 2 (12), 2 pieces.
        assert_eq!(list.len(), 6 + 1 + 2 + 44 + 13 + 12 + 2);
        assert!(list[0].label == "tile 34" && list.last().unwrap().label == "sea piece 41");
        let view = view(&sim).unwrap();
        // Chester's anchor through the board projection equals the model's.
        let chester = [-30.0, map::ANCHOR_HEIGHT, 40.0];
        let gs = duel_board::gs_screen(&view, chester);
        assert_eq!((gs[0].floor() as i32, gs[1].floor() as i32), (243, 135));
        assert_eq!(
            anchor_report(&sim)
                .iter()
                .find(|a| a.0 == 2)
                .map(|a| (a.1, a.2)),
            Some((243, 135))
        );
        let tile = &loaded.models[&34];
        assert!(
            tile.draws
                .iter()
                .any(|d| d.blend == Blend::Standard && d.alpha_chain)
        );
        assert!(
            loaded.models[&41]
                .draws
                .iter()
                .any(|d| d.blend == Blend::Additive)
        );
        assert!(loaded.text_images.contains_key(&map::CAPTION_MESSAGE));
        let items = layer_items(&sim);
        assert!(
            items
                .iter()
                .any(|i| matches!(i, LayerItem::Text { message: 2520, .. }))
        );
    }

    /// Object 30 of savestate 653-mgo (card 392 at Chester, motion 0 at
    /// t = 11, obj+0x10 (0, 0x8000, 0), obj+0x20 (0, 58542, 0), camera
    /// target (-30, 40)) through the map view: the emulator's VU1 list
    /// matrices put its 6,914 vertices at canvas x 217.3..288.2, y
    /// 118.9..225.2 (GS - 1792) [emu, story/the project notes
    #[test]
    #[ignore = "Requires the private models import (ROSES_DUEL_MODELS_MANIFEST)"]
    fn private_leader_figure_projects_like_the_emulated_frame() {
        let path =
            std::env::var_os("ROSES_DUEL_MODELS_MANIFEST").expect("ROSES_DUEL_MODELS_MANIFEST");
        let loaded = crate::duel_models::load_model(Path::new(&path), 392).unwrap();
        let mut scene =
            crate::duel_models::UnitScene::new(loaded, map::FIGURE_MOTION, (3, 3), 0, 1.0).unwrap();
        for _ in 0..10 {
            scene.tick();
        }
        assert_eq!(scene.clock.time, 11.0);
        let object = map::FigureObject {
            position: [-30.0, 24.0, 40.0],
            base_rotation: map::FIGURE_BASE_ROTATION,
            rotation: [0, 58542, 0],
            scale: [map::FIGURE_SCALE; 3],
            recentre: map::FIGURE_RECENTRE,
            alpha: 0x80,
            visible: true,
            heading: 25774,
            motion: map::FIGURE_MOTION,
        };
        place_figure(&mut scene, &object);
        let target = [-30.0, 40.0];
        let view = camera::look_at(map::camera_eye(target), [target[0], 0.0, target[1]]).unwrap();
        let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
        for mesh in &scene.frame().unwrap() {
            for p in &mesh.positions {
                let s = duel_board::gs_screen(&view, p.map(f64::from));
                for k in 0..2 {
                    min[k] = min[k].min(s[k]);
                    max[k] = max[k].max(s[k]);
                }
            }
        }
        let emulator = ([217.3, 118.9], [288.2, 225.2]);
        for k in 0..2 {
            assert!(
                (min[k] - emulator.0[k]).abs() < 0.2 && (max[k] - emulator.1[k]).abs() < 0.2,
                "{min:?} {max:?} vs {emulator:?}"
            );
        }
    }
}
