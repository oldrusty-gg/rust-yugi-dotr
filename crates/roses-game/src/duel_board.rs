//! Duel board scene model for the optional renderer (no GPU code here).
//!
//! Input is the private output of `roses import-duel-board` (manifest, raw RH0
//! geometry JSON, GS-byte PNG textures), hash-checked on load. This module
//! places the original models with the traced rules and runs the original
//! orbit camera. Sources are PAL EE addresses from the private board research
//! (`work/research/duel/claude-screen/board/NOTES.md`):
//!
//! - cell (c, r) origin = ((c-3)*50, -35 + h*35/128, (r-3)*50), terrain object
//!   rotation Y 0x8000 plus the model's node-zero Y word (0x1AF4C4, 0x1E2370);
//! - arena objects 0..2 at (0,-35,0); windows RH0 #(255 + 21*side + kind),
//!   side 0 at Y 0x6000 and 0xA000, side 1 at -0x2000 and 0x2000 (0x1AF6DC);
//! - board vertex colour = min(material maximum, 0.8*g*ambient), diffuse 0
//!   (object +0x120 = (0, 0.8, g)); g applied to the ambient term is [H];
//! - camera: target = ((c-3)*50, 40, (r-3)*50) of the active cursor, eye =
//!   target + 200*(0, sin 45°, cos 45°) for side 0 (0x1FC570, 0x1AE9F0),
//!   look-at 0x1FB1F0, GS projection x = 256 + 800x/z, y = 256 + 1074.2855y/z;
//! - cursor moves start a linear track of N = 8 updates (0x1DA360, 0x1FB430).
//!
//! Not modelled: overlays, units, cards, HUD, cursor tint pass, arena #3/#4,
//! the runtime spin of node-flag-0x8000 nodes (arena #2 backdrop, crush node
//! 0), crush extra nodes, raised cells (height > 0 box), side 1 camera input.
use crate::camera::{self, Mat4};
use crate::{Result, bounded_read, hash_valid, validate_relative_path};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MANIFEST_SCHEMA: &str = "roses-duel-board-import-2";
pub const CELLS: usize = 49;
pub const SIDE: u8 = 7;
/// First duel (Weevil) board: `parse_pal_rules(...).maps[2]`; the round-2
/// savestate's 49 cell terrain bytes equal it in row-major order.
pub const DEFAULT_MAP: usize = 2;
/// Cursor track length for a single cursor step (0x1D4114 and siblings).
pub const CURSOR_TRACK_FRAMES: u32 = 8;
/// Side-0 camera defaults from duel init 0x1AE9F0.
pub const CAMERA_DISTANCE: f64 = 200.0;
pub const CAMERA_YAW_SIDE0: u16 = 0x4000;
pub const CAMERA_PITCH: u16 = 0x2000;
pub const CAMERA_TARGET_Y: f64 = 40.0;
/// GS projection columns as stored in EE RAM (0x2ED7A0), column-major.
pub const GS_PROJECTION: Mat4 = [
    [800.0, 0.0, 0.0, 0.0],
    [0.0, 1_074.285_522_460_937_5, 0.0, 0.0],
    [2048.0, 2048.0, -2048.25, 1.0],
    [0.0, 0.0, 16_779_264.0, 0.0],
];
/// Window object slots 5 (side 0) and 6 (side 1): two Y rotations each.
pub const WINDOW_ANGLES: [[u16; 2]; 2] = [[0x6000, 0xA000], [0xE000, 0x2000]];
/// Leader card kinds of the first duel (round-2 savestate: #263 and #285).
pub const DEFAULT_WINDOW_KINDS: [u8; 2] = [8, 9];
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_GEOMETRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TEXTURE_BYTES: u64 = 4 * 1024 * 1024;

// ---------------------------------------------------------------- manifest
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub source: ManifestSource,
    pub geometry: FileRef,
    pub terrain_maps: TerrainMaps,
    pub models: Vec<ModelRef>,
    pub overlay_textures: OverlayTextures,
    pub gs_dither_matrices: DitherMatrices,
}
/// TEXSYS textures used by the unit layers (GS-byte PNGs like the models').
#[derive(Clone, Debug, Deserialize)]
pub struct OverlayTextures {
    /// TEXSYS #2: card frames and the card back (texels 256..512 x 0..368).
    pub card_frames: TextureRef,
    /// TEXSYS #5: leader emblems (side 0 left half, side 1 right half).
    pub emblems: TextureRef,
}
/// GS DIMX table read from the executable: [row y & 3][column x & 3].
#[derive(Clone, Debug, Deserialize)]
pub struct DitherMatrices {
    pub board: [[i8; 4]; 4],
    pub hud: [[i8; 4]; 4],
    pub overlay: [[i8; 4]; 4],
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
pub struct TerrainMaps {
    pub maps: Vec<Vec<u8>>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ModelRef {
    pub id: String,
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

// ---------------------------------------------------------------- geometry
#[derive(Clone, Debug, Deserialize)]
struct Geometry {
    schema_version: u32,
    archive_sha256: String,
    terrain: Vec<GeoModel>,
    arena: Vec<GeoModel>,
    windows: Vec<GeoModel>,
}
#[derive(Clone, Debug, Deserialize)]
struct GeoModel {
    role: String,
    code: Option<u8>,
    slot: Option<u8>,
    side: Option<u8>,
    leader_kind: Option<u8>,
    source_sha256: String,
    node_zero_scale: [f32; 3],
    node_zero_rotation_words: [u32; 3],
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
    texture_index: u16,
    material_words: [u32; 12],
    gs_registers: Vec<[u64; 2]>,
    matrix_slot: u8,
}
#[derive(Clone, Debug, Deserialize)]
struct GeoStrip {
    draw_index: u16,
    textured: bool,
    vertices: Vec<GeoVertex>,
}
#[derive(Clone, Debug, Deserialize)]
struct GeoVertex {
    texture_raw: [i16; 3],
    position_raw: [i16; 3],
}

/// One GPU draw: a draw group's strips with one texture-mapping mode.
#[derive(Clone, Debug)]
pub struct DrawMesh {
    pub draw_index: usize,
    pub textured: bool,
    pub positions_raw: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    /// Manifest texture path (hash-checked PNG in `LoadedBoard::textures`).
    pub texture: String,
    pub clamp_u: bool,
    pub clamp_v: bool,
    pub linear: bool,
    /// ATST GREATER reference when alpha testing is on (AFAIL KEEP).
    pub alpha_greater_than: Option<u8>,
    /// TEX0 TCC: texture alpha used (RGBA) or vertex alpha only (RGB).
    pub texture_alpha: bool,
    /// Ambient, diffuse, maximum vectors (V4_32 floats).
    pub material: [f32; 12],
    pub matrix_slot: u8,
}
#[derive(Clone, Debug)]
pub struct BoardModel {
    pub id: String,
    pub scale: [f64; 3],
    /// Node-zero Y rotation word (16-bit angle units).
    pub node_rotation_y: u16,
    pub draws: Vec<DrawMesh>,
}

#[derive(Debug)]
pub struct LoadedBoard {
    pub manifest: Manifest,
    pub manifest_sha256: String,
    pub models: BTreeMap<String, BoardModel>,
    /// Hash-checked PNG bytes by manifest path; kept in memory.
    pub textures: BTreeMap<String, Vec<u8>>,
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
        return Err("Board file resolves outside the import directory".into());
    }
    Ok(path)
}

/// GS register value by address from a draw's seven (value, register) pairs.
fn register(draw: &GeoDraw, address: u64) -> Result<u64> {
    draw.gs_registers
        .iter()
        .find(|pair| pair[1] == address)
        .map(|pair| pair[0])
        .ok_or_else(|| format!("GS register {address:#x} missing"))
}

/// Triangle list of one original triangle strip (no culling: the VU emits
/// every strip triangle of on-screen board strips, private dump check).
pub fn strip_indices(base: u32, count: u32) -> Vec<u32> {
    (2..count)
        .flat_map(|i| {
            let (a, b) = if i % 2 == 0 {
                (i - 2, i - 1)
            } else {
                (i - 1, i - 2)
            };
            [base + a, base + b, base + i]
        })
        .collect()
}

fn convert_model(model: &GeoModel, textures: &[TextureRef], id: &str) -> Result<BoardModel> {
    let mut draws = Vec::new();
    for (index, draw) in model.draws.iter().enumerate() {
        let clamp = register(draw, 0x08)?;
        let test = register(draw, 0x47)?;
        let tex1 = register(draw, 0x14)?;
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
        if (ate == 1 && (atst != 6 || afail != 0)) || zte != 1 || ztst != 3 || test & 0xc000 != 0 {
            return Err(format!("{id}: unsupported TEST register {test:#x}"));
        }
        let texture = model
            .textures
            .get(usize::from(draw.texture_index))
            .ok_or_else(|| format!("{id}: draw texture index out of range"))?;
        let tex0_high = *texture
            .raw_header_words
            .get(13)
            .ok_or_else(|| format!("{id}: texture header truncated"))?;
        // Stored TEX0 (RH2 header words 12/13): TCC bit 34, TFX bits 35-36.
        if (tex0_high >> 3) & 3 != 0 {
            return Err(format!("{id}: only TFX MODULATE is supported"));
        }
        let path = textures
            .iter()
            .find(|t| t.index == usize::from(draw.texture_index))
            .ok_or_else(|| format!("{id}: manifest lacks texture {}", draw.texture_index))?
            .path
            .clone();
        for textured in [true, false] {
            let mut mesh = DrawMesh {
                draw_index: index,
                textured,
                positions_raw: Vec::new(),
                uv: Vec::new(),
                indices: Vec::new(),
                texture: path.clone(),
                clamp_u: wms == 1,
                clamp_v: wmt == 1,
                linear: mmag == 1,
                alpha_greater_than: (ate == 1).then_some(aref as u8),
                texture_alpha: (tex0_high >> 2) & 1 == 1,
                material: draw.material_words.map(f32::from_bits),
                matrix_slot: draw.matrix_slot,
            };
            for strip in model
                .strips
                .iter()
                .filter(|s| usize::from(s.draw_index) == index && s.textured == textured)
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
                }
            }
            if !mesh.indices.is_empty() {
                if mesh.material.iter().any(|v| !v.is_finite()) {
                    return Err(format!("{id}: invalid material"));
                }
                draws.push(mesh);
            }
        }
    }
    let scale = model.node_zero_scale.map(f64::from);
    if scale.iter().any(|s| !s.is_finite() || *s <= 0.0) || draws.is_empty() {
        return Err(format!("{id}: invalid model scale or no draws"));
    }
    Ok(BoardModel {
        id: id.into(),
        scale,
        node_rotation_y: model.node_zero_rotation_words[1] as u16,
        draws,
    })
}

fn model_id(model: &GeoModel) -> Result<String> {
    Ok(
        match (
            model.role.as_str(),
            model.code,
            model.slot,
            model.side,
            model.leader_kind,
        ) {
            ("terrain", Some(c), None, None, None) => format!("terrain-{c}"),
            ("arena", None, Some(s), None, None) => format!("arena-{s}"),
            ("window", None, None, Some(s), Some(k)) => format!("window-{s}-{k}"),
            _ => return Err("Unknown board model role".into()),
        },
    )
}

/// Load and hash-check a board import. Every referenced file must lie inside
/// the manifest's directory.
pub fn load_board(manifest_path: &Path) -> Result<LoadedBoard> {
    let bytes = bounded_read(manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Board manifest: {e}"))?;
    if manifest.schema != MANIFEST_SCHEMA
        || ![
            &manifest.source.iso_sha256,
            &manifest.source.executable_sha256,
            &manifest.source.model_archive.sha256,
            &manifest.geometry.sha256,
        ]
        .into_iter()
        .all(|h| hash_valid(h))
    {
        return Err("Unsupported board manifest schema or fingerprints".into());
    }
    if manifest.terrain_maps.maps.is_empty()
        || manifest
            .terrain_maps
            .maps
            .iter()
            .any(|m| m.len() != CELLS || m.iter().any(|c| *c > 9))
    {
        return Err("Board manifest terrain maps are invalid".into());
    }
    let root = manifest_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let geometry_bytes = bounded_read(&child(&root, &manifest.geometry.path)?, MAX_GEOMETRY_BYTES)?;
    if sha(&geometry_bytes) != manifest.geometry.sha256 {
        return Err("Board geometry fingerprint mismatch".into());
    }
    let geometry: Geometry =
        serde_json::from_slice(&geometry_bytes).map_err(|e| format!("Board geometry: {e}"))?;
    if geometry.schema_version != 1
        || geometry.archive_sha256 != manifest.source.model_archive.sha256
    {
        return Err("Board geometry does not match the manifest source".into());
    }
    let refs: BTreeMap<&str, &ModelRef> =
        manifest.models.iter().map(|m| (m.id.as_str(), m)).collect();
    let mut models = BTreeMap::new();
    let mut textures = BTreeMap::new();
    let mut decoded = 0u64;
    for model in geometry
        .terrain
        .iter()
        .chain(&geometry.arena)
        .chain(&geometry.windows)
    {
        let id = model_id(model)?;
        let reference = refs
            .get(id.as_str())
            .ok_or_else(|| format!("Manifest lacks model {id}"))?;
        if reference.record_sha256 != model.source_sha256 {
            return Err(format!("{id}: record fingerprint mismatch"));
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
                return Err("Board texture budget exceeded".into());
            }
            textures.insert(texture.path.clone(), png);
        }
        models.insert(id.clone(), convert_model(model, &reference.textures, &id)?);
    }
    for code in 0..10 {
        if !models.contains_key(&format!("terrain-{code}")) {
            return Err(format!("Board import lacks terrain {code}"));
        }
    }
    let overlay = &manifest.overlay_textures;
    for (texture, size) in [
        (&overlay.card_frames, [512, 384]),
        (&overlay.emblems, [256, 128]),
    ] {
        let png = bounded_read(&child(&root, &texture.path)?, MAX_TEXTURE_BYTES)?;
        if !hash_valid(&texture.sha256)
            || sha(&png) != texture.sha256
            || png.get(..8) != Some(b"\x89PNG\r\n\x1a\n")
            || [texture.width, texture.height] != size
        {
            return Err(format!("Overlay texture mismatch: {}", texture.path));
        }
        textures.insert(texture.path.clone(), png);
    }
    let dither = &manifest.gs_dither_matrices;
    if [dither.board, dither.hud, dither.overlay]
        .iter()
        .flatten()
        .flatten()
        .any(|v| !(-4..=3).contains(v))
    {
        return Err("Board manifest dither matrices are out of range".into());
    }
    Ok(LoadedBoard {
        manifest,
        manifest_sha256: sha(&bytes),
        models,
        textures,
    })
}

// ---------------------------------------------------------------- lighting
/// Board vertex colour in GS units: RGB = floor(min(maximum, 0.8*g*ambient)),
/// A = diffuse lane 3 (128). Diffuse is zero for board objects.
pub fn vertex_color(material: &[f32; 12], brightness: f32) -> [f32; 4] {
    let lane = |i: usize| {
        (material[8 + i])
            .min(material[i] * 0.8 * brightness)
            .max(0.0)
            .floor()
    };
    [
        lane(0),
        lane(1),
        lane(2),
        material[7].clamp(0.0, 255.0).floor(),
    ]
}

// ---------------------------------------------------------------- placement
pub fn translation(t: [f64; 3]) -> Mat4 {
    let mut m = camera::identity();
    m[3] = [t[0], t[1], t[2], 1.0];
    m
}
pub fn scale(s: [f64; 3]) -> Mat4 {
    let mut m = camera::identity();
    for (i, v) in s.into_iter().enumerate() {
        m[i][i] = v;
    }
    m
}
fn rotation_y16(angle: u16) -> Mat4 {
    camera::rotation_y(angle as i16)
}
/// World matrix for raw int16 positions of a model placed at `position` with
/// object Y rotation `angle` (plus the model's node-zero Y word).
pub fn object_world(model: &BoardModel, position: [f64; 3], angle: u16) -> Mat4 {
    let rotation = rotation_y16(angle.wrapping_add(model.node_rotation_y));
    camera::mul(
        &camera::mul(&translation(position), &rotation),
        &scale(model.scale),
    )
}
pub fn cell_center(column: u8, row: u8) -> [f64; 2] {
    [
        (f64::from(column) - 3.0) * 50.0,
        (f64::from(row) - 3.0) * 50.0,
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub model: String,
    pub label: String,
    pub world: Mat4,
    /// Board cell of a terrain instance.
    pub cell: Option<(u8, u8)>,
}

#[derive(Clone, Debug)]
pub struct BoardLayout {
    pub terrain: [u8; CELLS],
    pub window_kinds: [u8; 2],
    /// Y angle of the arena #2 backdrop (its runtime spin is unresolved).
    pub backdrop_angle: u16,
    /// Cell heights (cell +0x14, clamped -128..128): the tile's origin y is
    /// -35 + H * 35 / 128 (field magic's c502 slides a tile down and up)
    /// [F 0x1AEF30 / 0x1E2370].
    pub heights: [i16; CELLS],
}

/// Instances in the original draw order (0x1AEF30): cursor cell, remaining
/// cells by terrain code row-major, arena 0..2, windows of slot 5 then 6.
pub fn instances(
    board: &LoadedBoard,
    layout: &BoardLayout,
    cursor: (u8, u8),
) -> Result<Vec<Instance>> {
    let mut out = Vec::new();
    let cursor_index = usize::from(cursor.1) * 7 + usize::from(cursor.0);
    let mut order = vec![cursor_index];
    for code in 0..10u8 {
        order.extend((0..CELLS).filter(|&i| i != cursor_index && layout.terrain[i] == code));
    }
    for index in order {
        let code = layout.terrain[index];
        let id = format!("terrain-{code}");
        let model = board.models.get(&id).ok_or("Terrain model missing")?;
        let (column, row) = ((index % 7) as u8, (index / 7) as u8);
        let [x, z] = cell_center(column, row);
        // Cell height: 0 in every surveyed state but c502's slide; the
        // +-24 x 35 box of H > 0 is not modelled (c502 never raises).
        let h = f64::from(i32::from(layout.heights[index].clamp(-128, 128)) * 35) / 128.0;
        out.push(Instance {
            model: id,
            label: format!("cell ({column},{row})"),
            world: object_world(model, [x, -35.0 + h, z], 0x8000),
            cell: Some((column, row)),
        });
    }
    for slot in 0..3 {
        let id = format!("arena-{slot}");
        let model = board.models.get(&id).ok_or("Arena model missing")?;
        let angle = if slot == 2 { layout.backdrop_angle } else { 0 };
        out.push(Instance {
            model: id,
            label: format!("arena {slot}"),
            world: object_world(model, [0.0, -35.0, 0.0], angle),
            cell: None,
        });
    }
    for (side, (kind, angles)) in layout.window_kinds.iter().zip(WINDOW_ANGLES).enumerate() {
        let id = format!("window-{side}-{kind}");
        let model = board
            .models
            .get(&id)
            .ok_or_else(|| format!("Window model {id} missing"))?;
        for angle in angles {
            out.push(Instance {
                model: id.clone(),
                label: format!("window slot {} at {angle:#06x}", 5 + side),
                world: object_world(model, [0.0, -35.0, 0.0], angle),
                cell: None,
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- camera
pub fn cursor_target(cursor: (u8, u8)) -> [f64; 3] {
    let [x, z] = cell_center(cursor.0, cursor.1);
    [x, CAMERA_TARGET_Y, z]
}
/// Orbit eye: target + dist * Ry(0x4000 - yaw) * Rx(-pitch) * (0,0,1). Exact
/// for the two side yaws; the x sign for other yaws is unverified [H].
pub fn orbit_eye(target: [f64; 3], distance: f64, yaw: u16, pitch: u16) -> [f64; 3] {
    let p = f64::from(pitch) * std::f64::consts::TAU / 65536.0;
    let a = f64::from(0x4000u16.wrapping_sub(yaw) as i16) * std::f64::consts::TAU / 65536.0;
    let (sp, cp) = p.sin_cos();
    let (sa, ca) = a.sin_cos();
    [
        target[0] + distance * sa * cp,
        target[1] + distance * sp,
        target[2] + distance * ca * cp,
    ]
}

/// Linear camera track (0x1FB430): value = start + elapsed/N * (end - start).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub frames: u32,
    pub elapsed: u32,
}
impl Track {
    pub fn value(&self) -> [f64; 3] {
        // A 0-update track snaps to its end (the unit camera's cut).
        if self.frames == 0 {
            return self.end;
        }
        let t = f64::from(self.elapsed.min(self.frames)) / f64::from(self.frames);
        std::array::from_fn(|i| self.start[i] + t * (self.end[i] - self.start[i]))
    }
}

/// Orbit eye from a float polar (camera object +0: distance, yaw and pitch
/// in radians; yaw pi/2 = side 0): the same orbit as [`orbit_eye`].
pub fn orbit_eye_polar(target: [f64; 3], polar: [f64; 3]) -> [f64; 3] {
    let [distance, yaw, pitch] = polar;
    let a = std::f64::consts::FRAC_PI_2 - yaw;
    let (sp, cp) = pitch.sin_cos();
    let (sa, ca) = a.sin_cos();
    [
        target[0] + distance * sa * cp,
        target[1] + distance * sp,
        target[2] + distance * ca * cp,
    ]
}

/// The side-0 polar (dist 200, yaw 0x4000, pitch 0x2000) in radians.
pub fn default_polar() -> [f64; 3] {
    let r = |v: u16| f64::from(v) * std::f64::consts::TAU / 65536.0;
    [CAMERA_DISTANCE, r(CAMERA_YAW_SIDE0), r(CAMERA_PITCH)]
}

/// A linear polar track (camera object polar, radians): the yaw takes the
/// wrapped difference in [-pi, pi) (K2: 1.571 -> 4.712 goes down through
/// 1.374 at the first update) [emu].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolarTrack {
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub frames: u32,
    pub elapsed: u32,
}
impl PolarTrack {
    pub fn value(&self) -> [f64; 3] {
        let t = f64::from(self.elapsed.min(self.frames)) / f64::from(self.frames.max(1));
        let tau = std::f64::consts::TAU;
        let mut yaw = (self.end[1] - self.start[1]).rem_euclid(tau);
        if yaw >= std::f64::consts::PI {
            yaw -= tau;
        }
        [
            self.start[0] + t * (self.end[0] - self.start[0]),
            self.start[1] + t * yaw,
            self.start[2] + t * (self.end[2] - self.start[2]),
        ]
    }
}

#[derive(Clone, Debug)]
pub struct BoardCamera {
    pub cursor: (u8, u8),
    pub target: [f64; 3],
    pub track: Option<Track>,
    /// Distance, yaw and pitch (radians).
    pub polar: [f64; 3],
    pub polar_track: Option<PolarTrack>,
    /// Projection distance / 800 (1 = the board's GS projection): a CMOT
    /// camera's field of view (the Exodia event's Summoned Lord camera).
    pub zoom: f64,
}
impl BoardCamera {
    /// Instant placement (0x1DFAE0): cursor, target and polar set at once.
    pub fn snapped(cursor: (u8, u8)) -> Self {
        Self {
            cursor,
            target: cursor_target(cursor),
            track: None,
            polar: default_polar(),
            polar_track: None,
            zoom: 1.0,
        }
    }

    /// Start tracks of the target and the polar toward `target` / `polar`
    /// over `frames` updates (0x1FDB60 / 0x1FB430), from the current values.
    pub fn track_to(&mut self, target: [f64; 3], polar: [f64; 3], frames: u32) {
        self.track = Some(Track {
            start: self.target,
            end: target,
            frames,
            elapsed: 0,
        });
        self.polar_track = Some(PolarTrack {
            start: self.polar,
            end: polar,
            frames,
            elapsed: 0,
        });
    }

    /// Whether a target or polar track is still running.
    pub fn moving(&self) -> bool {
        self.track.is_some() || self.polar_track.is_some()
    }
    /// One cursor step (clamped to the board). Starts a new N-frame track
    /// from the current (possibly mid-track) target. Returns whether it moved.
    pub fn step(&mut self, dc: i8, dr: i8) -> bool {
        let column = (self.cursor.0 as i8 + dc).clamp(0, SIDE as i8 - 1) as u8;
        let row = (self.cursor.1 as i8 + dr).clamp(0, SIDE as i8 - 1) as u8;
        if (column, row) == self.cursor {
            return false;
        }
        self.cursor = (column, row);
        self.track = Some(Track {
            start: self.target,
            end: cursor_target(self.cursor),
            frames: CURSOR_TRACK_FRAMES,
            elapsed: 0,
        });
        true
    }
    /// Advance one update (two PAL fields at 25 Hz).
    pub fn tick(&mut self) {
        if let Some(track) = &mut self.track {
            track.elapsed += 1;
            self.target = track.value();
            if track.elapsed >= track.frames {
                self.track = None;
            }
        }
        if let Some(track) = &mut self.polar_track {
            track.elapsed += 1;
            self.polar = track.value();
            if track.elapsed >= track.frames {
                self.polar = track.end;
                self.polar_track = None;
            }
        }
    }
    pub fn eye(&self) -> [f64; 3] {
        orbit_eye_polar(self.target, self.polar)
    }
    /// The view matrix. A camera that looks straight down (pitch 0x4000,
    /// the battle camera) gets the original's degenerate matrix
    /// ([`straight_down_view`]); every other pitch the usual look-at.
    pub fn view(&self) -> Option<Mat4> {
        if pitch_angle(self.polar[2]) == 0x4000 {
            return straight_down_view(self.target, self.polar[0]);
        }
        let view = camera::look_at(self.eye(), self.target)?;
        if self.zoom == 1.0 {
            return Some(view);
        }
        // P(d) V = P(800) S V with S scaling view x and y by d / 800.
        let mut scaled = view;
        for column in &mut scaled {
            column[0] *= self.zoom;
            column[1] *= self.zoom;
        }
        Some(scaled)
    }
}

/// 0x1FC1C0: the 16-bit pitch angle of a float pitch (radians),
/// trunc(32768 * pitch / pi) in f32 (EE `mul.s` rounds up, `div.s` to
/// nearest; both exact for pi/2).
pub fn pitch_angle(pitch: f64) -> i32 {
    let pi = std::f32::consts::PI;
    (32768.0f32 * pitch as f32 / pi) as i32
}

/// The view matrix the original builds for a straight-down camera: the eye
/// is (tx, ty + distance, tz) and the look direction (+0, -1, +0) (cos 0x4000
/// is exactly 0, so target - eye has zero x and z), which 0x20E160 turns
/// into fixed rows (-1, -1, 1), (0, 0, -1), (1, -1, 0) and an eye-dependent
/// translation through PCSX2's x / 0 = +-FLT_MAX ([F 0x20CD50/0x20B550,
/// bit-exact against the routine run with PCSX2 float rules; footage state
/// 010 RAM]).
pub fn straight_down_view(target: [f64; 3], distance: f64) -> Option<Mat4> {
    let eye = [
        target[0] as f32,
        (target[1] as f32) + distance as f32,
        target[2] as f32,
    ];
    let m =
        roses_assets::opening_scene::ee_view::view_from_direction(eye, [0.0, -1.0, 0.0]).ok()??;
    Some(m.map(|row| row.map(f64::from)))
}

/// Backend clip matrix for the board shader: GS clip = P*V*W followed by the
/// adaptation x' = (x - 2047.5 w)/256, y' = -(y - 2047.5 w)/256 and w' = w.
/// The shader writes (x', y', 1, w'): depth 1/w keeps GS depth order (Z24 =
/// 16779264/w - 2048.25, ZTST GREATER). The half-pixel term puts GS pixel
/// (x, y), sampled at integer coordinates, at the GPU pixel centre.
pub fn ndc_from_source(view: &Mat4, world: &Mat4) -> Mat4 {
    let clip = camera::mul(&camera::mul(&GS_PROJECTION, view), world);
    let adapt: Mat4 = [
        [1.0 / 256.0, 0.0, 0.0, 0.0],
        [0.0, -1.0 / 256.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0],
        [-2047.5 / 256.0, 2047.5 / 256.0, 0.0, 1.0],
    ];
    camera::mul(&adapt, &clip)
}
/// Backend clip matrix for positions already in GS window coordinates
/// (x, y with the 1792 offset, w = 1): the adaptation of
/// [`ndc_from_source`] alone.
pub fn ndc_from_gs_window() -> Mat4 {
    [
        [1.0 / 256.0, 0.0, 0.0, 0.0],
        [0.0, -1.0 / 256.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0],
        [-2047.5 / 256.0, 2047.5 / 256.0, 0.0, 1.0],
    ]
}
/// GS screen position (raw 512 canvas, XYOFFSET removed) of a world point.
pub fn gs_screen(view: &Mat4, world_point: [f64; 3]) -> [f64; 3] {
    let clip = camera::mul(&GS_PROJECTION, view);
    let c: [f64; 4] = std::array::from_fn(|r| {
        clip[0][r] * world_point[0]
            + clip[1][r] * world_point[1]
            + clip[2][r] * world_point[2]
            + clip[3][r]
    });
    [c[0] / c[3] - 1792.0, c[1] / c[3] - 1792.0, c[2] / c[3]]
}

/// Parse `--terrain` as 49 digits 0..9 (row-major, row 0 first).
pub fn parse_terrain(value: &str) -> Result<[u8; CELLS]> {
    let digits: Vec<u8> = value
        .bytes()
        .filter(|b| !matches!(b, b',' | b' ' | b'/'))
        .map(|b| {
            b.is_ascii_digit()
                .then(|| b - b'0')
                .ok_or_else(|| "Terrain must be 49 digits 0..9".to_string())
        })
        .collect::<Result<_>>()?;
    digits
        .try_into()
        .map_err(|_| "Terrain must be exactly 49 cells".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: &Mat4, p: [f64; 3]) -> [f64; 4] {
        std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r])
    }
    fn model(rotation: u16, scale: f64) -> BoardModel {
        BoardModel {
            id: "m".into(),
            scale: [scale; 3],
            node_rotation_y: rotation,
            draws: Vec::new(),
        }
    }

    #[test]
    fn straight_down_camera_uses_the_original_degenerate_view() {
        // Footage state 010 (battle camera, target (0, 40, 25), yaw pi): the
        // view matrix PCSX2 left at 0x2ED760.
        let r = |v: u16| f64::from(v) * std::f64::consts::TAU / 65536.0;
        let mut camera = BoardCamera::snapped((3, 3));
        camera.target = [0.0, 40.0, 25.0];
        camera.polar = [150.0, r(0x8000), r(0x4000)];
        assert_eq!(pitch_angle(camera.polar[2]), 0x4000);
        let view = camera.view().unwrap();
        let want = [
            [-1.0, -1.0, 1.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [1.0, -1.0, 0.0, 0.0],
            [-1.0, 1.0, 1.0, 1.0],
        ];
        assert_eq!(view, want);
        // One update before the end of the track the pitch is not 0x4000
        // and the ordinary look-at applies.
        camera.polar[2] = r(0x3E00);
        assert_ne!(camera.view().unwrap(), want);
    }

    #[test]
    fn side_zero_camera_matches_the_round2_state() {
        // Round-2 savestate: cursor (3,6), eye (0,181.421,291.421), target
        // (0,40,150), view column 3 = (0,-77.7818,334.3503).
        let camera = BoardCamera::snapped((3, 6));
        assert_eq!(camera.target, [0.0, 40.0, 150.0]);
        let eye = camera.eye();
        assert!(
            eye[0].abs() < 1e-9
                && (eye[1] - 181.421).abs() < 1e-3
                && (eye[2] - 291.421).abs() < 1e-3
        );
        let view = camera.view().unwrap();
        assert!((view[3][1] + 77.781_754).abs() < 1e-4 && (view[3][2] - 334.350_281).abs() < 1e-4);
        let half = std::f64::consts::FRAC_1_SQRT_2;
        assert!((view[1][1] + half).abs() < 1e-6 && (view[2][2] + half).abs() < 1e-6);
        // Side 1 yaw 0xC000 mirrors the eye to -z.
        let e1 = orbit_eye([0.0, 40.0, 0.0], 200.0, 0xC000, CAMERA_PITCH);
        assert!(e1[0].abs() < 1e-9 && (e1[2] + 141.421).abs() < 1e-3);
    }

    #[test]
    fn cursor_steps_follow_a_linear_eight_frame_track() {
        let mut camera = BoardCamera::snapped((3, 6));
        assert!(camera.step(1, 0));
        for frame in 1..=8 {
            camera.tick();
            assert!((camera.target[0] - 50.0 * f64::from(frame) / 8.0).abs() < 1e-9);
        }
        assert!(camera.track.is_none());
        camera.tick();
        assert_eq!(camera.target, [50.0, 40.0, 150.0]);
        // Clamped at the edge; a new step mid-track starts from the current value.
        assert!(!camera.step(0, 1));
        assert!(camera.step(0, -1));
        camera.tick();
        camera.tick();
        assert!(camera.step(0, -1));
        let start = camera.target[2];
        assert!((start - (150.0 - 2.0 * 50.0 / 8.0)).abs() < 1e-9);
        camera.tick();
        assert!((camera.target[2] - (start + (50.0 - start) / 8.0)).abs() < 1e-9);
    }

    #[test]
    fn placement_reproduces_captured_draw_matrices() {
        // Forest (node Y 0) at cell (3,6): rows diag(-s, s, -s), t (0,-35,150).
        let s = 0.000_930_595;
        let forest = object_world(&model(0, s), [0.0, -35.0, 150.0], 0x8000);
        assert!((forest[0][0] + s).abs() < 1e-12 && (forest[2][2] + s).abs() < 1e-12);
        assert!(forest[2][0].abs() < 1e-15 && forest[3] == [0.0, -35.0, 150.0, 1.0]);
        // Wasteland node word 0x7FFF: 0x8000 + 0x7FFF = -1 unit, observed
        // row 0 = (s, 0, -1e-7) at 7 decimals.
        let w = 0.000_732_601;
        let waste = object_world(&model(0x7fff, w), [150.0, -35.0, 0.0], 0x8000);
        assert!((waste[0][0] - w).abs() < 1e-9);
        assert!((waste[2][0] * 1e7).round() == -1.0 && (waste[0][2] * 1e7).round() == 1.0);
        // Window slot 6 at -0x2000: rows (0.0170219, 0, -0.0170219).
        let window = object_world(&model(0, 0.024_072_612), [0.0, -35.0, 0.0], 0xE000);
        assert!(
            (window[0][0] - 0.017_021_9).abs() < 1e-7 && (window[2][0] + 0.017_021_9).abs() < 1e-7
        );
    }

    #[test]
    fn board_colour_uses_the_minimum_of_maximum_and_scaled_ambient() {
        let tile = [
            127.0, 127.0, 127.0, 0.0, 200.0, 200.0, 200.0, 128.0, 255.0, 255.0, 255.0, 0.0,
        ];
        assert_eq!(vertex_color(&tile, 1.0), [101.0, 101.0, 101.0, 128.0]);
        let dark = [
            102.0, 102.0, 102.0, 0.0, 12.0, 12.0, 12.0, 128.0, 24.0, 24.0, 24.0, 0.0,
        ];
        assert_eq!(vertex_color(&dark, 1.0), [24.0, 24.0, 24.0, 128.0]);
        let surround = [
            51.0, 51.0, 51.0, 0.0, 76.0, 76.0, 76.0, 128.0, 152.0, 152.0, 152.0, 0.0,
        ];
        assert_eq!(vertex_color(&surround, 1.0)[0], 40.0);
        assert_eq!(vertex_color(&tile, 0.5)[0], 50.0);
    }

    #[test]
    fn projection_adapter_matches_gs_screen_coordinates() {
        let camera = BoardCamera::snapped((3, 6));
        let view = camera.view().unwrap();
        let world = translation([10.0, -34.0, 120.0]);
        let gs = gs_screen(&view, [10.0, -34.0, 120.0]);
        let c = apply(&ndc_from_source(&view, &world), [0.0, 0.0, 0.0]);
        let (x, y) = (c[0] / c[3], c[1] / c[3]);
        // GPU pixel centre = GS integer sample + 0.5.
        assert!(((x + 1.0) * 256.0 - (gs[0] + 0.5)).abs() < 1e-6);
        assert!(((1.0 - y) * 256.0 - (gs[1] + 0.5)).abs() < 1e-6);
        // Board centre projects to the canvas centre column.
        assert!((gs_screen(&view, [0.0, 40.0, 150.0])[0] - 256.0).abs() < 1e-9);
    }

    #[test]
    fn strips_become_lists_and_terrain_parses() {
        assert_eq!(strip_indices(10, 4), [10, 11, 12, 12, 11, 13]);
        assert!(strip_indices(0, 2).is_empty());
        let t = parse_terrain(&"0".repeat(49)).unwrap();
        assert_eq!(t, [0; 49]);
        assert!(parse_terrain(&"0".repeat(48)).is_err());
        assert!(parse_terrain(&format!("{}a", "0".repeat(48))).is_err());
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;

    #[test]
    #[ignore = "Requires the private board import (ROSES_DUEL_BOARD_MANIFEST)"]
    fn private_import_loads_and_orders_the_first_duel_board() {
        let path =
            std::env::var_os("ROSES_DUEL_BOARD_MANIFEST").expect("ROSES_DUEL_BOARD_MANIFEST");
        let board = load_board(Path::new(&path)).unwrap();
        let terrain: [u8; CELLS] = board.manifest.terrain_maps.maps[DEFAULT_MAP]
            .clone()
            .try_into()
            .unwrap();
        let layout = BoardLayout {
            terrain,
            heights: [0; CELLS],
            window_kinds: DEFAULT_WINDOW_KINDS,
            backdrop_angle: 0,
        };
        let list = instances(&board, &layout, (3, 6)).unwrap();
        assert_eq!(list.len(), 49 + 3 + 4);
        assert_eq!(list[0].label, "cell (3,6)");
        // Map 2: wasteland at (6,3), (1,5), (4,5); drawn after all forest cells.
        let waste: Vec<_> = list
            .iter()
            .filter(|i| i.model == "terrain-1")
            .map(|i| i.label.as_str())
            .collect();
        assert_eq!(waste, ["cell (6,3)", "cell (1,5)", "cell (4,5)"]);
        assert_eq!(
            (list[45].model.as_str(), list[46].model.as_str()),
            ("terrain-0", "terrain-1")
        );
        let forest = &board.models["terrain-0"];
        let colors: Vec<_> = forest
            .draws
            .iter()
            .map(|d| vertex_color(&d.material, 1.0))
            .collect();
        assert_eq!(
            colors,
            [[101.0, 101.0, 101.0, 128.0], [0.0, 0.0, 0.0, 128.0]]
        );
        assert!(forest.draws[0].textured && !forest.draws[1].textured);
        let arena = &board.models["arena-0"];
        assert!(
            arena
                .draws
                .iter()
                .any(|d| d.alpha_greater_than == Some(0x3f))
        );
    }
}
