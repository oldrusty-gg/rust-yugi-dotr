//! The battle scene's arena (battle state 0x39 phases 2-5): the terrain's
//! stage models from `roses import-duel-arena`, the StageCam camera path and
//! the units' arena poses.
//!
//! Traced [F/emu, `claude-units/ARENA-SCOPE.md`]:
//! - Phase 2 loads MODEL.MRG bank block 22 (object 7, shared) and block
//!   23 + terrain (objects 8..); the terrain is the battle cell's. The
//!   stage geometry is in world space at the origin.
//! - Phase 3 starts the terrain's StageCam path (STAGECAM.MRG, one CMOT per
//!   terrain): a camera track of kind 8 over frames - 1 updates, evaluated
//!   by 0x1FCB50 at frame = (frames - 1) * elapsed / duration with linear
//!   keys (0x1FD060 / 0x1FD260); the fov gives the projection distance
//!   d = 256 / tan(fov / 2) (camera +0x44).
//! - The attacker stands at (0, 50, 80) facing yaw 0x8000, the defender at
//!   (0, 50, -80) facing yaw 0 [emu K2 153].
use crate::campaign_map::{MapModel, TextureRef};
use crate::camera::Mat4;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

pub const MANIFEST_SCHEMA: &str = "roses-duel-arena-import-1";
pub const ATTACKER_POSITION: [f64; 3] = [0.0, 50.0, 80.0];
pub const DEFENDER_POSITION: [f64; 3] = [0.0, 50.0, -80.0];
pub const ATTACKER_YAW: i32 = 0x8000;
pub const DEFENDER_YAW: i32 = 0;

#[derive(Deserialize)]
struct FileRef {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
struct ModelRef {
    slot: u16,
    record_sha256: String,
    textures: Vec<TextureRef>,
}
#[derive(Deserialize)]
struct BlockRef {
    block: u32,
    terrain: Option<u32>,
    geometry: FileRef,
    models: Vec<ModelRef>,
    unsupported: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct Slices {
    offsets: Vec<u32>,
}
#[derive(Deserialize)]
struct Source {
    model_archive: Archive,
}
#[derive(Deserialize)]
struct Archive {
    sha256: String,
}
#[derive(Deserialize)]
struct Manifest {
    schema: String,
    source: Source,
    blocks: Vec<BlockRef>,
    stagecam: FileRef,
    stagecam_slices: Slices,
    /// Absent in imports made before the attack phase was ported.
    #[serde(default)]
    unit_tables: Option<UnitTables>,
    /// BMODULE.MRG and the executable; absent in older imports.
    #[serde(default)]
    effects: Option<Effects>,
    environments: Option<Environments>,
}
#[derive(Deserialize)]
struct Environments {
    raw_words: Vec<Vec<u32>>,
}

/// A terrain's light for the arena's units (environment record 0x290910 +
/// terrain * 0xC0, ARENA-ENVIRONMENT.md 1): the unit scales at +0x0C and
/// light 0 of the light set at +0x20 (colour at +4, s32 angles at +0x10;
/// direction = Rz Ry Rx (0, 0, 1)) [F/emu].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArenaLight {
    pub colour: [f32; 3],
    pub angles: [i32; 3],
    /// S_d * g and S_a * g.
    pub diffuse: f32,
    pub ambient: f32,
    /// Lights 1 / 2 have colour (terrain 6): not reproduced (reported).
    pub extra_lights: bool,
    /// Height fog (+0xB0 Q0, Q1; off when equal) and FOGCOL (+0xB8 bytes
    /// b0 b1 b2 -> R = b2, G = b1, B = b0) [F/emu ARENA-ENVIRONMENT.md].
    pub fog: Option<([f32; 2], [u8; 3])>,
}

impl ArenaLight {
    /// The fog uniforms: (Q0, Q1, on, 0) and FOGCOL bytes.
    pub fn fog_uniforms(&self) -> ([f32; 4], [f32; 4]) {
        match self.fog {
            Some((q, c)) => ([q[0], q[1], 1.0, 0.0], [f32::from(c[0]), f32::from(c[1]), f32::from(c[2]), 0.0]),
            None => ([0.0; 4], [0.0; 4]),
        }
    }

    fn from_words(w: &[u32]) -> Option<Self> {
        let f = |i: usize| w.get(i).map(|v| f32::from_bits(*v));
        let (d, a, g) = (f(3)?, f(4)?, f(5)?);
        let light = |i: usize| -> Option<[f32; 3]> { Some([f(8 + 12 * i + 1)?, f(8 + 12 * i + 2)?, f(8 + 12 * i + 3)?]) };
        let angles = [w.get(12)?, w.get(13)?, w.get(14)?].map(|v| *v as i32);
        let extra = (1..3).any(|i| light(i).is_some_and(|c| c.iter().any(|v| *v != 0.0)));
        let (q0, q1, c) = (f(44)?, f(45)?, *w.get(46)?);
        let fog = (q0 != q1).then_some(([q0, q1], [(c >> 16) as u8, (c >> 8) as u8, c as u8]));
        Some(Self {
            fog,
            colour: light(0)?,
            angles,
            diffuse: d * g,
            ambient: a * g,
            extra_lights: extra,
        })
    }
}
/// The stage objects' (types 2 / 3) light of a terrain: the record's stage
/// scales (+0x00 S_d, S_a; g is overwritten every update by 0x2DF5B2 / 128
/// = 1.0) and light 0 of its set [F ARENA-ENVIRONMENT.md 1, 3, 5].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageLight {
    pub light: roses_client::monster_pose::UnitLight,
}

impl StageLight {
    fn from_words(w: &[u32], sine: &roses_client::monster_pose::SineTable) -> Option<Self> {
        let f = |i: usize| w.get(i).map(|v| f64::from(f32::from_bits(*v)));
        let colour = [f(9)?, f(10)?, f(11)?];
        let angles = [w.get(12)?, w.get(13)?, w.get(14)?].map(|v| *v as i32);
        let r = sine.rotation(angles);
        Some(Self {
            light: roses_client::monster_pose::UnitLight {
                direction: [r[2][0], r[2][1], r[2][2]],
                colour,
                diffuse_scale: f(0)?,
                ambient_scale: f(1)?,
            },
        })
    }

    /// The light of stage object `slot` in terrain `t`: the sea's object 12
    /// has its own scales (1.0, 0.1) [F ARENA-ENVIRONMENT.md 4].
    pub fn object(&self, t: u32, slot: u16) -> roses_client::monster_pose::UnitLight {
        let mut light = self.light;
        if t == 4 && slot == 12 {
            light.diffuse_scale = 1.0;
            light.ambient_scale = f64::from(0.1f32);
        }
        light
    }
}

/// A sine table from host floats, for imports without the executable.
fn float_sine() -> roses_client::monster_pose::SineTable {
    let values = (0..1025)
        .map(|i| if i == 1024 { 1.0 } else { (f64::from(i) * std::f64::consts::FRAC_PI_2 / 1024.0).sin() as f32 })
        .collect();
    roses_client::monster_pose::SineTable::new(values).expect("1025 values from 0 to 1")
}

#[derive(Deserialize)]
struct Effects {
    bmodule: FileRef,
    executable: FileRef,
    /// CMODULE.MRG; absent in imports made before the duel opening's c620.
    #[serde(default)]
    cmodule: Option<FileRef>,
}
#[derive(Deserialize)]
struct UnitTables {
    size: Values,
    flags: Values,
}
#[derive(Deserialize)]
struct Values {
    values: Vec<u16>,
}

/// A loaded arena import.
pub struct LoadedArena {
    /// The shared piece (block 22, object 7), lit for each terrain.
    pub shared: BTreeMap<u32, Vec<MapModel>>,
    /// Stage models by terrain code.
    pub stages: BTreeMap<u32, Vec<MapModel>>,
    /// Records the grammar did not cover, by terrain (reported).
    pub unsupported: BTreeMap<u32, usize>,
    /// Hash-checked texture PNGs by manifest path.
    pub textures: BTreeMap<String, Vec<u8>>,
    /// Camera paths by terrain.
    pub cameras: BTreeMap<u32, Cmot>,
    /// Per-card unit size (0x2CEC28) and camera flags (0x2CE6D0); empty
    /// in older imports (the attack phase is then not played).
    pub unit_sizes: Vec<u16>,
    pub unit_flags: Vec<u16>,
    /// The units' light by terrain (from the environment records).
    pub lights: BTreeMap<u32, ArenaLight>,
    /// The stage objects' light by terrain (the same records).
    pub stage_lights: BTreeMap<u32, StageLight>,
    /// (BMODULE.MRG, executable), hash-checked, when imported.
    pub effects: Option<std::sync::Arc<(Vec<u8>, Vec<u8>)>>,
    /// CMODULE.MRG, hash-checked, when imported.
    pub cmodule: Option<std::sync::Arc<Vec<u8>>>,
}

/// What the attack phase needs at the orbit's end.
pub struct ArenaSetup {
    pub units: [std::sync::Arc<crate::duel_arena_battle::ArenaUnitData>; 2],
    /// The stage path's last camera (where sub 1 starts).
    pub stage_end: CmotCamera,
}

/// Loads the attack phase's data for (terrain, [attacker card, defender card]).
pub struct ArenaProvider {
    /// The setup for (terrain, [attacker card, defender card]).
    pub setup: Box<dyn Fn(u8, [u16; 2]) -> Result<ArenaSetup, String> + Send + Sync>,
    /// Start running the cards' attack effects in the background (at the
    /// arena's start, so the setup at the orbit's end does not wait).
    pub prefetch: Box<dyn Fn([u16; 2]) + Send + Sync>,
    /// The attack effects' live inputs for [attacker card, defender card]
    /// and the duel import's TEXEFF.MRG (`None` without the effects part).
    pub effect_inputs: Box<dyn Fn([u16; 2], Option<&[u8]>) -> Option<crate::duel_bmodule::EffectInputs> + Send + Sync>,
    /// CMODULE.MRG (the duel start's c620), when imported.
    pub cmodule: Option<std::sync::Arc<Vec<u8>>>,
    /// The models import's manifest (models outside the battle: the Exodia
    /// event's Summoned Lord Exodia, `duel_exodia`).
    pub models_manifest: std::path::PathBuf,
}

/// Attack effect returns by card, filled by background runs.
type EffectCache = std::sync::Arc<std::sync::Mutex<BTreeMap<u16, std::sync::Arc<std::sync::OnceLock<Option<Vec<i32>>>>>>>;

fn effect_slot(cache: &EffectCache, card: u16) -> (std::sync::Arc<std::sync::OnceLock<Option<Vec<i32>>>>, bool) {
    let mut map = cache.lock().unwrap_or_else(|e| e.into_inner());
    match map.get(&card) {
        Some(slot) => (slot.clone(), false),
        None => {
            let slot = std::sync::Arc::new(std::sync::OnceLock::new());
            map.insert(card, slot.clone());
            (slot, true)
        }
    }
}

/// Run `card`'s attack effect headless (handles: attacker side 1 slot 1,
/// defender side 0 slot 1 as in the footage; the traced modules do not
/// read them).
fn run_effect(effects: &(Vec<u8>, Vec<u8>), card: u16, own: u16, target: u16) -> Option<Vec<i32>> {
    match roses_ee::effects::attack_effect_returns(&effects.1, &effects.0, card, target, own) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Duel arena: unsupported, not played: card {card}'s attack effect ({e})");
            None
        }
    }
}

/// A provider over a loaded arena and a models import (its MONSTER.MRG
/// copy gives the models and their CMOT camera records).
pub fn arena_provider(arena: &LoadedArena, models_manifest: std::path::PathBuf) -> Option<ArenaProvider> {
    if arena.unit_sizes.is_empty() || arena.unit_flags.is_empty() {
        return None;
    }
    let cameras = arena.cameras.clone();
    let (sizes, flags) = (arena.unit_sizes.clone(), arena.unit_flags.clone());
    let effects = arena.effects.clone();
    let cache: EffectCache = Default::default();
    let (prefetch_cache, prefetch_effects) = (cache.clone(), effects.clone());
    let prefetch = Box::new(move |cards: [u16; 2]| {
        let Some(effects) = prefetch_effects.clone() else {
            return;
        };
        for (i, card) in cards.into_iter().enumerate() {
            let (slot, fresh) = effect_slot(&prefetch_cache, card);
            if fresh {
                let effects = effects.clone();
                let (own, target) = if i == 0 { (0x101, 0x001) } else { (0x001, 0x101) };
                std::thread::spawn(move || {
                    let _ = slot.set(run_effect(&effects, card, own, target));
                });
            }
        }
    });
    let effect_models = models_manifest.clone();
    let manifest_for_lord = models_manifest.clone();
    let setup = Box::new(move |terrain, cards: [u16; 2]| {
        let stage = cameras
            .get(&u32::from(terrain))
            .ok_or_else(|| format!("no StageCam path for terrain {terrain}"))?;
        // The attack effect's returns, run headless from the disc's own
        // program [handles: attacker side 1 slot 1, defender side 0 slot 1
        // as in the footage; the two traced modules do not read them].
        let effect = |card: u16, own: u16, target: u16| -> Option<Vec<i32>> {
            let effects = effects.as_deref()?;
            let (slot, _) = effect_slot(&cache, card);
            // A background run (prefetch) finishes first; else run here.
            slot.get_or_init(|| run_effect(effects, card, own, target)).clone()
        };
        let unit = |card: u16| -> Result<std::sync::Arc<crate::duel_arena_battle::ArenaUnitData>, String> {
            let i = usize::from(card);
            let (size, flag) = match (sizes.get(i), flags.get(i)) {
                (Some(s), Some(f)) => (*s, *f),
                _ => return Err(format!("card {card} is outside the unit tables")),
            };
            let model = crate::duel_models::load_model(&models_manifest, card)?;
            let cmots = crate::duel_models::unit_cameras(&models_manifest, card)?;
            let parse = |b: Option<Vec<u8>>| b.map(|b| Cmot::parse(&b)).transpose();
            Ok(std::sync::Arc::new(crate::duel_arena_battle::ArenaUnitData {
                card,
                size,
                flags: flag,
                model,
                attack_camera: parse(cmots.0)?,
                victory_camera: parse(cmots.1)?,
                effect: None,
            }))
        };
        let mut a = unit(cards[0])?;
        let mut d = unit(cards[1])?;
        if let Some(u) = std::sync::Arc::get_mut(&mut a) {
            u.effect = effect(cards[0], 0x101, 0x001);
        }
        if let Some(u) = std::sync::Arc::get_mut(&mut d) {
            u.effect = effect(cards[1], 0x001, 0x101);
        }
        Ok(ArenaSetup {
            units: [a, d],
            stage_end: stage.at(stage.frames.saturating_sub(1), stage.frames.saturating_sub(1)),
        })
    });
    let program = arena.effects.clone();
    let models = effect_models;
    let effect_inputs = Box::new(move |cards: [u16; 2], texeff: Option<&[u8]>| {
        let program = program.clone()?;
        let tex = cards.map(|c| match crate::duel_models::effect_textures(&models, c) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Duel arena: card {c}'s effect textures are not available ({e})");
                Vec::new()
            }
        });
        let own = tex.clone().map(|t| t.into_iter().map(|t| t.map(|t| t.rh2)).collect());
        let own_images = tex.map(|t| std::sync::Arc::new(t.into_iter().map(|t| t.and_then(|t| t.image)).collect()));
        Some(crate::duel_bmodule::EffectInputs { program, texeff: texeff.map(crate::duel_bmodule::texeff_records).unwrap_or_default(), own, own_images })
    });
    let cmodule = arena.cmodule.clone();
    Some(ArenaProvider { setup, prefetch, effect_inputs, cmodule, models_manifest: manifest_for_lord })
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_checked(root: &Path, file: &FileRef, limit: u64) -> Result<Vec<u8>, String> {
    if file.path.contains("..") || file.path.starts_with('/') || file.path.contains(':') {
        return Err(format!("arena file path {} leaves the import", file.path));
    }
    let bytes = crate::bounded_read(&root.join(&file.path), limit)?;
    if sha(&bytes) != file.sha256 {
        return Err(format!("{} does not match its import hash", file.path));
    }
    Ok(bytes)
}

/// The import's CMODULE.MRG and SLES_515.79 alone (the story's c401), when
/// imported; `Ok(None)` for an import without them.
pub fn load_cmodule(manifest_path: &Path) -> Result<Option<(Vec<u8>, Vec<u8>)>, String> {
    let bytes = crate::bounded_read(manifest_path, 4 * 1024 * 1024)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("arena manifest: {e}"))?;
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(format!("unsupported arena import {}", manifest.schema));
    }
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let Some(e) = &manifest.effects else { return Ok(None) };
    let Some(c) = &e.cmodule else { return Ok(None) };
    Ok(Some((
        read_checked(root, &e.executable, 8 * 1024 * 1024)?,
        read_checked(root, c, 40 * 1024 * 1024)?,
    )))
}

pub fn load_arena(manifest_path: &Path) -> Result<LoadedArena, String> {
    let bytes = crate::bounded_read(manifest_path, 4 * 1024 * 1024)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("arena manifest: {e}"))?;
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(format!("unsupported arena import {}", manifest.schema));
    }
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let mut out = LoadedArena {
        shared: BTreeMap::new(),
        stages: BTreeMap::new(),
        unsupported: BTreeMap::new(),
        textures: BTreeMap::new(),
        cameras: BTreeMap::new(),
        unit_sizes: manifest.unit_tables.as_ref().map(|t| t.size.values.clone()).unwrap_or_default(),
        unit_flags: manifest.unit_tables.as_ref().map(|t| t.flags.values.clone()).unwrap_or_default(),
        effects: None,
        cmodule: None,
        stage_lights: BTreeMap::new(),
        lights: manifest
            .environments
            .as_ref()
            .map(|e| {
                e.raw_words
                    .iter()
                    .enumerate()
                    .filter_map(|(t, w)| ArenaLight::from_words(w).map(|l| (t as u32, l)))
                    .collect()
            })
            .unwrap_or_default(),
    };
    if let Some(e) = &manifest.effects {
        out.effects = Some(std::sync::Arc::new((
            read_checked(root, &e.bmodule, 32 * 1024 * 1024)?,
            read_checked(root, &e.executable, 8 * 1024 * 1024)?,
        )));
        if let Some(c) = &e.cmodule {
            out.cmodule = Some(std::sync::Arc::new(read_checked(root, c, 40 * 1024 * 1024)?));
        }
    }
    // The stage objects' light (types 2 / 3) per terrain: the record's
    // stage scales and light set, angles through the executable's sine
    // table when imported [F ARENA-ENVIRONMENT.md 1, 3, 4].
    let sine = match &out.effects {
        Some(e) => roses_assets::parse_pal_sine_table(&e.1).ok().and_then(|v| roses_client::monster_pose::SineTable::new(v).ok()),
        None => None,
    }
    .unwrap_or_else(float_sine);
    let stage_lights: BTreeMap<u32, StageLight> = manifest
        .environments
        .as_ref()
        .map(|e| {
            e.raw_words
                .iter()
                .enumerate()
                .filter_map(|(t, w)| StageLight::from_words(w, &sine).map(|l| (t as u32, l)))
                .collect()
        })
        .unwrap_or_default();
    for block in &manifest.blocks {
        let geometry = read_checked(root, &block.geometry, 64 * 1024 * 1024)?;
        let refs: BTreeMap<u16, (String, Vec<TextureRef>)> = block
            .models
            .iter()
            .map(|m| (m.slot, (m.record_sha256.clone(), m.textures.clone())))
            .collect();
        for model in &block.models {
            for t in &model.textures {
                if !out.textures.contains_key(&t.path) {
                    let png = read_checked(
                        root,
                        &FileRef {
                            path: t.path.clone(),
                            sha256: t.sha256.clone(),
                        },
                        4 * 1024 * 1024,
                    )?;
                    out.textures.insert(t.path.clone(), png);
                }
            }
        }
        let build = |t: Option<u32>| {
            let light = t.and_then(|t| stage_lights.get(&t).map(|l| (t, l)));
            crate::campaign_map::models_from_geometry(
                &geometry,
                &manifest.source.model_archive.sha256,
                &refs,
                &|slot| match light {
                    Some((t, l)) => l.object(t, slot),
                    // Older imports without environment records.
                    None => crate::campaign_map::MAP_LIGHT,
                },
            )
        };
        match block.terrain {
            None if block.block == 22 => {
                for t in 0..stage_lights.len().max(1) as u32 {
                    out.shared.insert(t, build(Some(t))?);
                }
            }
            Some(t) => {
                out.unsupported.insert(t, block.unsupported.len());
                out.stages.insert(t, build(Some(t))?);
            }
            None => return Err(format!("arena block {} has no role", block.block)),
        }
    }
    out.stage_lights = stage_lights;
    let stagecam = read_checked(root, &manifest.stagecam, 64 * 1024)?;
    let offsets = &manifest.stagecam_slices.offsets;
    for t in 0..offsets.len().saturating_sub(1) {
        let (a, b) = (offsets[t] as usize, offsets[t + 1] as usize);
        if let Some(slice) = stagecam.get(a..b) {
            if let Ok(cmot) = Cmot::parse(slice) {
                out.cameras.insert(t as u32, cmot);
            }
        }
    }
    Ok(out)
}

/// A CMOT camera path: frame count and sparse keys (frame, value).
#[derive(Clone, Debug, PartialEq)]
pub struct Cmot {
    pub frames: u32,
    pub eye: Vec<(u32, [f32; 3])>,
    pub target: Vec<(u32, [f32; 3])>,
    pub roll: Vec<(u32, f32)>,
    pub fov: Vec<(u32, f32)>,
}

/// The camera a CMOT gives for one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CmotCamera {
    pub eye: [f64; 3],
    pub target: [f64; 3],
    pub roll: f64,
    /// Projection distance 256 / tan(fov / 2) (camera +0x44).
    pub distance: f64,
}

impl Cmot {
    /// Header "CMOT", 0, byte size, frame count; then four (offset, count)
    /// u16 pairs (eye, target, roll, fov) indexing 8-byte units after 0x20;
    /// vector keys are (u32 frame, 3 f32), scalar keys (u32 frame, f32).
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        if b.get(..4) != Some(b"CMOT") || b.len() < 0x20 {
            return Err("not a CMOT record".into());
        }
        let u32_at = |o: usize| -> Result<u32, String> {
            b.get(o..o + 4)
                .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
                .ok_or_else(|| "CMOT truncated".to_string())
        };
        let f32_at = |o: usize| u32_at(o).map(f32::from_bits);
        let u16_at = |o: usize| -> Result<usize, String> {
            b.get(o..o + 2)
                .map(|s| usize::from(u16::from_le_bytes(s.try_into().unwrap())))
                .ok_or_else(|| "CMOT truncated".to_string())
        };
        let frames = u32_at(12)?;
        let ch: Vec<usize> = (0..8).map(|i| u16_at(0x10 + 2 * i)).collect::<Result<_, _>>()?;
        if ch.iter().any(|&v| v > 4096) || frames == 0 || frames > 100_000 {
            return Err("CMOT header out of bounds".into());
        }
        let vec = |o: usize, n: usize| -> Result<Vec<(u32, [f32; 3])>, String> {
            (0..n)
                .map(|i| {
                    let at = 0x20 + o * 8 + 16 * i;
                    Ok((u32_at(at)?, [f32_at(at + 4)?, f32_at(at + 8)?, f32_at(at + 12)?]))
                })
                .collect()
        };
        let sca = |o: usize, n: usize| -> Result<Vec<(u32, f32)>, String> {
            (0..n)
                .map(|i| {
                    let at = 0x20 + o * 8 + 8 * i;
                    Ok((u32_at(at)?, f32_at(at + 4)?))
                })
                .collect()
        };
        let c = Cmot {
            frames,
            eye: vec(ch[0], ch[1])?,
            target: vec(ch[2], ch[3])?,
            roll: sca(ch[4], ch[5])?,
            fov: sca(ch[6], ch[7])?,
        };
        if c.eye.is_empty() || c.target.is_empty() || c.fov.is_empty() {
            return Err("CMOT without eye, target or fov keys".into());
        }
        Ok(c)
    }

    fn interp<const N: usize>(keys: &[(u32, [f32; N])], t: f64) -> [f64; N] {
        for w in keys.windows(2) {
            let (f0, f1) = (f64::from(w[0].0), f64::from(w[1].0));
            if f1 > t {
                if f0 > t {
                    return w[0].1.map(f64::from);
                }
                let (a, b) = ((f1 - t) / (f1 - f0), (t - f0) / (f1 - f0));
                return std::array::from_fn(|i| a * f64::from(w[0].1[i]) + b * f64::from(w[1].1[i]));
            }
        }
        keys.last().map(|k| k.1.map(f64::from)).unwrap_or([0.0; N])
    }

    /// The camera after `elapsed` of `duration` updates (0x1FCB50:
    /// frame = (frames - 1) * elapsed / duration).
    pub fn at(&self, elapsed: u32, duration: u32) -> CmotCamera {
        let t = f64::from(self.frames.saturating_sub(1)) * f64::from(elapsed) / f64::from(duration.max(1));
        let one = |k: &[(u32, f32)]| -> Vec<(u32, [f32; 1])> { k.iter().map(|(f, v)| (*f, [*v])).collect() };
        let roll = if self.roll.is_empty() { 0.0 } else { Self::interp(&one(&self.roll), t)[0] };
        let fov = Self::interp(&one(&self.fov), t)[0];
        CmotCamera {
            eye: Self::interp(&self.eye, t),
            target: Self::interp(&self.target, t),
            roll,
            distance: 256.0 / (fov.to_radians() / 2.0).tan(),
        }
    }
}

/// GS projection for a camera distance d (the duel's GS_PROJECTION with
/// 800 replaced by d; y keeps the PAL pixel aspect 1074.2855 / 800).
pub fn projection(distance: f64) -> Mat4 {
    let mut p = crate::duel_board::GS_PROJECTION;
    p[0][0] = distance;
    p[1][1] = distance * crate::duel_board::GS_PROJECTION[1][1] / 800.0;
    p
}

/// Backend clip matrix for a world matrix under an arena camera.
pub fn ndc(camera: &CmotCamera, world: &Mat4) -> Option<Mat4> {
    let view = crate::camera::look_at(camera.eye, camera.target)?;
    let clip = crate::camera::mul(&crate::camera::mul(&projection(camera.distance), &view), world);
    let adapt: Mat4 = [
        [1.0 / 256.0, 0.0, 0.0, 0.0],
        [0.0, -1.0 / 256.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0],
        [-2047.5 / 256.0, 2047.5 / 256.0, 0.0, 1.0],
    ];
    Some(crate::camera::mul(&adapt, &clip))
}

/// A stage model's world matrix (stage geometry is in world space: node 0
/// scale, rotation and translation only).
pub fn stage_world(model: &MapModel) -> Mat4 {
    crate::campaign_map::placed_rotated(model, [0.0; 3], model.node_rotation_y as i16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmot_keys_interpolate_linearly() {
        let c = Cmot {
            frames: 11,
            eye: vec![(0, [0.0, 80.0, 590.0]), (10, [100.0, 80.0, 490.0])],
            target: vec![(0, [0.0, 20.0, 0.0])],
            roll: vec![],
            fov: vec![(0, 41.54)],
        };
        let a = c.at(5, 10);
        assert!((a.eye[0] - 50.0).abs() < 1e-4 && (a.eye[2] - 540.0).abs() < 1e-4);
        assert!((a.distance - 675.0).abs() < 1.0, "{}", a.distance);
    }
}

#[cfg(test)]
mod private_tests {
    /// The environment records give the traced unit lights
    /// (ARENA-ENVIRONMENT.md 4); needs `ROSES_ARENA_MANIFEST`.
    #[test]
    #[ignore = "needs a private arena import"]
    fn environment_records_give_the_traced_unit_lights() {
        let arena = super::load_arena(std::path::Path::new(&std::env::var("ROSES_ARENA_MANIFEST").unwrap())).unwrap();
        let forest = arena.lights[&0];
        assert_eq!(forest.colour, [0.5; 3]);
        assert!((forest.diffuse - 0.8).abs() < 1e-6 && (forest.ambient - 0.64).abs() < 1e-6);
        let waste = arena.lights[&1];
        assert_eq!(waste.colour, [1.0, 0.5, 0.3]);
        assert!((waste.diffuse - 2.4).abs() < 1e-5 && (waste.ambient - 0.32).abs() < 1e-6);
        assert!(!forest.extra_lights && arena.lights[&6].extra_lights);
        assert_eq!(forest.fog, Some(([-100.0, 1500.0], [192, 255, 224])));
        assert_eq!(waste.fog, None);
        // Stage objects: forest (0.8, 0.8) under 0.5 grey from (0, 1, 0);
        // mountain unlit (S_d 0); the sea's object 12 (1.0, 0.1).
        let stage = arena.stage_lights[&0].light;
        assert_eq!((stage.diffuse_scale as f32, stage.ambient_scale as f32), (0.8, 0.8));
        assert!((stage.direction[1] - 1.0).abs() < 1e-6 && stage.colour == [0.5; 3]);
        let waste = arena.stage_lights[&1].light;
        assert!((waste.direction[0] - 0.866).abs() < 1e-3 && (waste.direction[1] - 0.5).abs() < 1e-3);
        assert_eq!(arena.stage_lights[&2].light.diffuse_scale, 0.0);
        let sea = arena.stage_lights[&4].object(4, 12);
        assert_eq!((sea.diffuse_scale, sea.ambient_scale as f32), (1.0, 0.1));
        assert_eq!(arena.shared.len(), arena.stage_lights.len());
    }

    /// Footage attack cameras (ARENA-SCOPE.md: Kumootoko card 406 CMOT 261
    /// frames, Orion card 370 265 frames [emu]); needs a private models
    /// import at `ROSES_MODELS_MANIFEST`.
    #[test]
    #[ignore = "needs a private models import"]
    fn footage_attack_cameras_have_the_traced_lengths() {
        let manifest = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        for (card, frames) in [(406u16, 261u32), (370, 265)] {
            let bytes = crate::duel_models::attack_camera(&manifest, card).unwrap().unwrap();
            let cmot = super::Cmot::parse(&bytes).unwrap();
            eprintln!("card {card}: {} frames, eye keys {}, fov keys {}", cmot.frames, cmot.eye.len(), cmot.fov.len());
            assert_eq!(cmot.frames, frames, "card {card}");
        }
    }
}
