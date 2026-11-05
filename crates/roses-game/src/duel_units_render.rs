//! Unit models on the duel board (MONSTER.MRG through `duel_models`, drawn
//! with the `--duel-model` materials).
//!
//! Which units have a model [F, board NOTES "Units"]: leaders (objects
//! 30 + side) and monsters in slots 1–5 (objects 20 + 5·side + slot − 1);
//! a face-down monster's model has scale 0 (not drawn). Magic and trap
//! slots have no model object.
//!
//! Where they are drawn [F]: the unit under the cursor is rendered after
//! the HUD into the 0x2000 buffer, Z-tested only against the HUD, and
//! composited over the colour buffer with an alpha-bit test, so the board
//! never hides it; the other units use normal depth. Here the cursor unit
//! goes to a cleared unit canvas of its own (render layer 2) that the
//! present pass composites over the board and under the HUD; the others are
//! drawn in the board canvas after the board draws with Z test GREATER.
//!
//! Not reproduced (reported): the composite's one-update lag (the original
//! shows the previous update's render), the alpha bit written by blended
//! draws (kept from the opaque pass here [H]), the position of the other
//! units' draws within the frame [H: after the board, before the card
//! quads], motions other than idle (attack, damage, destruction), unit
//! movement between cells (models jump), summon grow-in and the near-camera
//! fade.
use crate::duel_model_render::{
    ModelMaterial, ModelTextures, dither_mat4, mesh_of, model_material, to_mat4,
};
use bevy::{asset::UntypedAssetId, camera::visibility::RenderLayers, prelude::*};
use roses_game::camera;
use roses_game::duel_board;
use roses_game::duel_models::{self, LoadedModel, UnitScene};
use roses_game::duel_overlay::UnitInput;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Render layer of the cursor unit's canvas.
pub const UNIT_LAYER: usize = 2;
/// Sort depth of the first model group in the board canvas: after the board
/// draws (`board_order`, from −100000) and before the overlays (`order`,
/// from −1000).
const FIRST_GROUP_DEPTH: f32 = -50_000.0;
/// Sort slots reserved per unit (groups beyond are reported, not drawn).
const GROUPS_PER_UNIT: usize = 1024;

/// The battle arena's unit poses, attacker then defender: 0x1E64E0 re-poses
/// the unit objects at x 0, z +80 / -80, yaw 0x8000 / 0 (table 0x291440)
/// [emu ARENA-SCOPE.md "Units": footage objects 25 and 20].
pub const ARENA_POSES: [([f32; 3], i32); 2] = [([0.0, 50.0, 80.0], 0x8000), ([0.0, 50.0, -80.0], 0)];
/// Sort depth of the arena units' first group on the versus canvas (after
/// the stage draws, from -100000; before the cards and effects).
const ARENA_FIRST_DEPTH: f32 = -50_000.0;

/// Where an arena unit stands this update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArenaPose {
    pub card: u16,
    pub position: [f32; 3],
    pub yaw: i32,
    /// The battle's motion clock (`None`: idle on the renderer's own clock).
    pub clock: Option<roses_client::monster_pose::MotionClock>,
    /// A motion change being blended (previous motion, its time, weight).
    pub blend: Option<(u8, f32, f32)>,
    /// The terrain's light (None: the board's).
    pub light: Option<roses_game::duel_arena::ArenaLight>,
    /// obj+0x12F: 128 opaque; below it the model is drawn on the fade
    /// canvas and composited at alpha / 128; 0 = not drawn [F
    /// C621-DESTROY.md 7].
    pub alpha: u8,
    /// Drawn into the attack effect's unit target this update (0x1F0FD0).
    pub rt: bool,
    /// The clip matrix of that render when the effect's pre-model callback
    /// switched the camera (0x1EF8F0 t3); `None`: the arena camera.
    pub rt_ndc: Option<camera::Mat4>,
    /// The attack effect's writes to the unit object (tint, colour, scale).
    pub writes: Option<roses_fx::types::UnitWrites>,
    /// The stage light 0x2DF5B2 (128 = 1.0): FOGCOL is written with it.
    pub stage_light: u8,
}

impl ArenaPose {
    /// obj+0x12F as the model draw reads it: the battle's fade, or the
    /// attack effect's own write.
    pub fn shown_alpha(&self) -> u8 {
        match self.writes {
            Some(w) if w.tint[3] != 0x80 => self.alpha.min(w.tint[3]),
            _ => self.alpha,
        }
    }

    /// The composite colour (R, G, B) = obj+0x12E, +0x12D, +0x12C when an
    /// effect tints the unit (the draw then goes through the offscreen
    /// target like a fade) [F 0x1EFD64 / 0x1F0860].
    pub fn tint(&self) -> Option<[u8; 3]> {
        let w = self.writes?;
        let rgb = [w.tint[2], w.tint[1], w.tint[0]];
        (rgb != [0x80; 3]).then_some(rgb)
    }

    /// The full-window colour pass over the unit in its offscreen target
    /// (0x1F0708, when obj+0x1F8 bits 2-3 and +0x133 are not 0): colour (R, G,
    /// B) = (+0x132, +0x131, +0x130), alpha = +0x133 * +0x128 truncated, and
    /// the blend index of 0x208450's table (0 = 0x44, 1 = 0x48, 2 = 0x42, 3 =
    /// 0x84); it leaves the target's alpha alone (FBMSK 0xFF000000) [F
    /// 0x1EFD08 / 0x1F06C0..0x1F07F4; emu the project notes emu-pass draw 54].
    pub fn colour_pass(&self) -> Option<([u8; 4], u8)> {
        let w = self.writes?;
        let mode = (w.mode >> 2) & 3;
        if mode == 0 || w.add[3] == 0 {
            return None;
        }
        let alpha = (f32::from(w.add[3]) * w.fade).max(0.0) as u32 as u8;
        Some(([w.add[2], w.add[1], w.add[0], alpha], mode))
    }

    /// The composite's blend index (obj+0x1F8 bits 0-1 through 0x208450:
    /// 0 = ALPHA 0x44, 1 = 0x48, 2 = 0x42, 3 = 0x84) [F 0x1F0848].
    pub fn composite_mode(&self) -> u8 {
        self.writes.map_or(0, |w| w.mode & 3)
    }

    /// Whether the model goes through the offscreen target and a composite
    /// (0x1EFD08..0x1EFD90: the colour pass, a composite mode, or a
    /// multiplier byte other than 0x80).
    pub fn composited(&self) -> bool {
        self.shown_alpha() < 128 || self.tint().is_some() || self.colour_pass().is_some() || self.composite_mode() != 0
    }
}

/// Render layer of the arena's fade canvas (a fading unit drawn alone).
pub const FADE_LAYER: usize = 5;
/// The arena units' own fade canvases by role (the original's unit target
/// is drawn and composited per unit, 0x1EFD08..0x1F0860 inside each unit's
/// draw [F]; role 0 shares the board's canvas).
pub const FADE_LAYERS: [usize; 2] = [FADE_LAYER, 13];
/// Render layers of the attack effects' unit targets (role 0, 1).
pub const UNIT_RT_LAYERS: [usize; 2] = [8, 9];
/// The attack effects' draws into a unit target before its model (0x1EF8F0's
/// pre-model callback): a camera that clears the target and draws them first.
pub const UNIT_RT_PRE_LAYERS: [usize; 2] = [14, 15];

/// A unit model standing in the battle arena.
struct ArenaUnit {
    /// 0 attacker, 1 defender.
    role: usize,
    card: u16,
    scene: UnitScene,
    textures: ModelTextures,
    entities: Vec<Entity>,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<ModelMaterial>>,
    /// The same groups drawn into the effect's target with another camera
    /// (`ArenaPose::rt_ndc`), made on first use.
    rt_entities: Vec<Entity>,
    rt_materials: Vec<Handle<ModelMaterial>>,
}

/// What decides a unit's model: its object, card and side (a move only
/// changes where the model stands).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitKey {
    /// Original object index (20 + 5·side + slot − 1, or 30 + side): the
    /// draw order between units.
    pub object: u8,
    pub card: u16,
    pub side: u8,
}

/// A unit that shows a model and where it stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelUnit {
    pub key: UnitKey,
    pub cell: (u8, u8),
    /// A walk's drawn offset (world x/z).
    pub offset: [f32; 2],
    /// Yaw offset (record +0x30).
    pub turn: i32,
    /// The presentation's model level (unit +0x53, 128 = shown).
    pub level: u8,
    /// A module's model scale (c560's shrink).
    pub scale: Option<f32>,
}

/// The units that show a model, in object order.
pub fn model_units(units: &[UnitInput]) -> Vec<ModelUnit> {
    let mut wanted: Vec<ModelUnit> = units
        .iter()
        .filter_map(|u| {
            let object = match u.slot {
                0 => 30 + u.side,
                1..=5 if u.face_up => 20 + 5 * u.side + u.slot - 1,
                _ => return None,
            };
            Some(ModelUnit {
                key: UnitKey {
                    object,
                    card: u.card,
                    side: u.side,
                },
                cell: (u.cell[0], u.cell[1]),
                offset: u.offset,
                turn: u.turn,
                level: u.alpha.unwrap_or(128),
                scale: u.model_scale,
            })
        })
        .collect();
    wanted.sort_by_key(|w| w.key);
    wanted
}

struct ActiveUnit {
    key: UnitKey,
    /// The unit's cell (the destination while it walks [H: the original
    /// commits the cell on arrival at each step]).
    cell: (u8, u8),
    scene: UnitScene,
    textures: ModelTextures,
    entities: Vec<Entity>,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<ModelMaterial>>,
    cursor: bool,
}

/// The board's unit models (inserted with the duel scene when the game
/// import has a models import).
#[derive(Resource)]
pub struct UnitModels {
    manifest: PathBuf,
    gs16: bool,
    cache: BTreeMap<u16, Option<LoadedModel>>,
    active: Vec<ActiveUnit>,
    applied_view: Option<camera::Mat4>,
    /// Headless updates each wanted unit has existed for before the first
    /// frame (`count_update`), applied to its motion clock when built.
    pending: BTreeMap<UnitKey, u32>,
    /// The battle arena's two units (attacker, defender) while it shows.
    arena: Vec<ArenaUnit>,
    arena_cards: Option<[u16; 2]>,
    /// Near-camera fade per unit: (alpha U+0x53, direction).
    fade: BTreeMap<UnitKey, (u8, i8)>,
    /// The units' model level from the presentation (field magic's and the
    /// effects' fades of unit +0x53; 128 = shown).
    levels: BTreeMap<UnitKey, u8>,
    /// Appear amounts (unit +0x53) of models that appeared after the first
    /// sync: scale a / 128, +8 per update [F/emu SUMMON-PRESENTATION.md 5].
    appear: BTreeMap<UnitKey, u8>,
    seeded: bool,
    pub reported: BTreeSet<String>,
    /// A model outside the board's units (the Exodia event's Summoned Lord
    /// Exodia, object 0x20): its scene, meshes, materials and entities.
    free: Option<ArenaUnit>,
}

/// Where a free model stands this update: card, scene state (pose, motion
/// clock, blend and scale as the presentation keeps them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreePose {
    pub card: u16,
    pub position: [f32; 3],
    pub yaw: i32,
    pub scale: f32,
    pub clock: roses_client::monster_pose::MotionClock,
    pub motion: u8,
    pub blend: Option<(u8, f32, f32)>,
}

/// Images, meshes and materials created by a sync, for capture readiness.
#[derive(Default)]
pub struct Created {
    pub textures: Vec<AssetId<Image>>,
    pub meshes: Vec<AssetId<Mesh>>,
    pub materials: Vec<UntypedAssetId>,
}

impl UnitModels {
    /// The models import's manifest.
    pub fn manifest(&self) -> &std::path::Path {
        &self.manifest
    }

    pub fn new(manifest: PathBuf, gs16: bool) -> Self {
        Self {
            manifest,
            gs16,
            cache: BTreeMap::new(),
            active: Vec::new(),
            applied_view: None,
            pending: BTreeMap::new(),
            arena: Vec::new(),
            arena_cards: None,
            fade: BTreeMap::new(),
            levels: BTreeMap::new(),
            appear: BTreeMap::new(),
            seeded: false,
            free: None,
            reported: BTreeSet::new(),
        }
    }

    /// One update of the near-camera fade [F/emu HAND-SLIDE-AND-FADE.md 2]:
    /// a leader or face-up monster whose cell point (50 (c - 3), 40,
    /// 50 (r - 3)) is closer than 80 to the camera eye fades out by 8 per
    /// update; farther, it fades back in by 8 to 128.
    pub fn fade_update(&mut self, eye: [f64; 3], units: &[UnitInput]) {
        let wanted = model_units(units);
        self.fade.retain(|k, _| wanted.iter().any(|w| w.key == *k));
        for w in &wanted {
            let p = [
                50.0 * (f64::from(w.cell.0) - 3.0),
                40.0,
                50.0 * (f64::from(w.cell.1) - 3.0),
            ];
            let d = ((p[0] - eye[0]).powi(2) + (p[1] - eye[1]).powi(2) + (p[2] - eye[2]).powi(2)).sqrt();
            let (alpha, dir) = self.fade.entry(w.key).or_insert((128, 0));
            *dir = if d < 80.0 {
                -1
            } else if *alpha < 128 {
                1
            } else {
                0
            };
            *alpha = (i32::from(*alpha) + 8 * i32::from(*dir)).clamp(0, 128) as u8;
        }
    }

    /// The alpha a fading board unit is composited with (the lowest of the
    /// units between 0 and 128 [H: one fade canvas]).
    pub fn fade_composite_alpha(&self) -> Option<u8> {
        self.active
            .iter()
            .map(|u| self.unit_alpha(u.key))
            .filter(|a| *a > 0 && *a < 128)
            .min()
    }

    /// A model's alpha: the near-camera fade and the presentation's level.
    fn unit_alpha(&self, key: UnitKey) -> u8 {
        let fade = self.fade.get(&key).map_or(128, |f| f.0);
        fade.min(self.levels.get(&key).copied().unwrap_or(128))
    }

    /// The battle arena's units: `cards` (attacker, defender) stand at
    /// [`ARENA_POSES`] in their idle motion on `layer`, clipped by `ndc`
    /// (the arena camera, world to clip); `None` removes them.
    /// Not reproduced (reported): the terrain's environment lighting (units
    /// are lit as on the board) and every motion but idle.
    #[allow(clippy::too_many_arguments)]
    pub fn sync_arena(
        &mut self,
        commands: &mut Commands,
        poses: Option<[ArenaPose; 2]>,
        ndc: &camera::Mat4,
        layer: usize,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<ModelMaterial>,
        mut created: Option<&mut Created>,
    ) {
        let cards = poses.map(|p| p.map(|p| p.card));
        let fresh = cards != self.arena_cards;
        if fresh {
            for unit in self.arena.drain(..) {
                for entity in unit.entities.into_iter().chain(unit.rt_entities) {
                    commands.entity(entity).despawn();
                }
            }
            self.arena_cards = cards;
        }
        let (Some(cards), Some(poses)) = (cards, poses) else {
            return;
        };
        let ndc = to_mat4(ndc);
        if fresh {
            self.report("battle arena: the terrain's height fog on units and stage, and the stage's lighting".into());
            for (index, card) in cards.into_iter().enumerate() {
                let Some(loaded) = self.loaded(card) else {
                    continue;
                };
                let mut scene = match UnitScene::new(loaded, 0, (3, 3), index as u8, 1.0) {
                    Ok(scene) => scene,
                    Err(error) => {
                        self.report(format!("card {card}: {error}"));
                        continue;
                    }
                };
                scene.set_pose(poses[index].position, poses[index].yaw);
                if let Some(light) = &poses[index].light {
                    scene.set_arena_light(light);
                    if light.extra_lights {
                        self.report("battle arena: lights 1 and 2 of this terrain (only light 0 lights the units)".into());
                    }
                }
                if let Some(clock) = poses[index].clock {
                    scene.motion = clock.motion;
                    scene.clock = clock;
                }
                scene.blend = poses[index].blend;
                let frame = match scene.frame() {
                    Ok(frame) => frame,
                    Err(error) => {
                        self.report(format!("card {card}: {error}"));
                        continue;
                    }
                };
                if std::env::var_os("ROSES_BMODULE_LOG").is_some() {
                    let (mut lo, mut hi) = ([f32::MAX; 4], [f32::MIN; 4]);
                    for m in &frame {
                        for p in &m.positions {
                            let c = ndc * Vec4::new(p[0], p[1], p[2], 1.0);
                            for (k, v) in [c.x / c.w, c.y / c.w, c.z, c.w].into_iter().enumerate() {
                                lo[k] = lo[k].min(v);
                                hi[k] = hi[k].max(v);
                            }
                        }
                    }
                    eprintln!("arena unit {index} card {card}: alpha {} motion {} t {} groups {} clip bbox {lo:?} .. {hi:?}", poses[index].alpha, scene.motion, scene.clock.time, frame.len());
                }
                let dither = dither_mat4(&scene.loaded.dither);
                let mut unit = ArenaUnit {
                    role: index,
                    card,
                    textures: ModelTextures::default(),
                    entities: Vec::new(),
                    meshes: Vec::new(),
                    materials: Vec::new(),
                    rt_entities: Vec::new(),
                    rt_materials: Vec::new(),
                    scene,
                };
                let mut failed = None;
                for (group, mesh) in unit.scene.groups.iter().zip(&frame) {
                    let mut new_textures = Vec::new();
                    let texture = match unit.textures.get(images, &unit.scene.loaded, group, &mut new_textures) {
                        Ok(texture) => texture,
                        Err(error) => {
                            failed = Some(error);
                            break;
                        }
                    };
                    let mesh = meshes.add(mesh_of(mesh));
                    let material = materials.add(model_material(&group.state, ndc, texture, dither, self.gs16));
                    if let Some(c) = created.as_deref_mut() {
                        c.textures.extend(new_textures);
                        c.meshes.push(mesh.id());
                        c.materials.push(material.id().untyped());
                    }
                    unit.meshes.push(mesh);
                    unit.materials.push(material);
                }
                if let Some(error) = failed {
                    self.report(format!("card {card}: {error}"));
                    continue;
                }
                for (order, (mesh, material)) in unit.meshes.iter().zip(&unit.materials).enumerate() {
                    let depth = ARENA_FIRST_DEPTH + (index * GROUPS_PER_UNIT + order) as f32;
                    unit.entities.push(
                        commands
                            .spawn((
                                Name::new(format!("arena unit {index} card {card} group {order}")),
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(material.clone()),
                                Transform::from_xyz(0.0, 0.0, depth)
                                    .with_scale(Vec3::splat(duel_models::DRAW_ORDER_SCALE)),
                                bevy::camera::visibility::NoFrustumCulling,
                                RenderLayers::layer(layer),
                            ))
                            .id(),
                    );
                }
                self.arena.push(unit);
            }
            // (Then the per-update state below: layers, fades, scale.)
        }
        let mut errors = Vec::new();
        for unit in &mut self.arena {
            let pose = poses[unit.role];
            let (fog, mut colour) = pose.light.map_or(([0.0; 4], [0.0; 4]), |l| l.fog_uniforms());
            for c in &mut colour[..3] {
                *c = (*c * f32::from(pose.stage_light) / 128.0).floor();
            }
            for handle in &unit.materials {
                if let Some(m) = materials.get(handle) {
                    if m.fog_state() != (fog, colour) {
                        if let Some(mut m) = materials.get_mut(handle) {
                            m.set_fog(fog, colour);
                        }
                    }
                }
            }
            if pose.position != unit.scene.object.position || pose.yaw != unit.scene.object.rotation[1] {
                unit.scene.set_pose(pose.position, pose.yaw);
            }
            match pose.clock {
                Some(clock) => {
                    unit.scene.motion = clock.motion;
                    unit.scene.clock = clock;
                }
                None if !fresh => unit.scene.tick(),
                None => {}
            }
            unit.scene.blend = pose.blend;
            // obj+0x30: the effect's scale (x, y, z) [run: sweep writes].
            let scale = pose.writes.map_or([1.0; 3], |w| [w.scale[0], w.scale[1], w.scale[2]]);
            if unit.scene.object.scale != scale {
                unit.scene.set_scale3(scale);
            }
            let alpha = pose.shown_alpha();
            let mut layers = Vec::new();
            if alpha > 0 {
                layers.push(if pose.composited() { FADE_LAYERS[unit.role & 1] } else { layer });
            }
            if pose.rt && pose.rt_ndc.is_none() {
                layers.push(UNIT_RT_LAYERS[unit.role & 1]);
            }
            for entity in &unit.entities {
                commands.entity(*entity).insert((
                    if layers.is_empty() { Visibility::Hidden } else { Visibility::Inherited },
                    RenderLayers::from_layers(&layers),
                ));
            }
            // The render into the target with the callback's camera: copies
            // of the groups on the target layer with that clip matrix.
            let rt_ndc = pose.rt_ndc.filter(|_| pose.rt).map(|m| to_mat4(&m));
            if rt_ndc.is_some() && unit.rt_entities.is_empty() {
                for (order, (mesh, material)) in unit.meshes.iter().zip(&unit.materials).enumerate() {
                    let Some(copy) = materials.get(material).cloned() else { continue };
                    let copy = materials.add(copy);
                    let depth = ARENA_FIRST_DEPTH + (unit.role * GROUPS_PER_UNIT + order) as f32;
                    unit.rt_entities.push(
                        commands
                            .spawn((
                                Name::new(format!("arena unit {} target copy group {order}", unit.role)),
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(copy.clone()),
                                Transform::from_xyz(0.0, 0.0, depth).with_scale(Vec3::splat(duel_models::DRAW_ORDER_SCALE)),
                                bevy::camera::visibility::NoFrustumCulling,
                                RenderLayers::layer(UNIT_RT_LAYERS[unit.role & 1]),
                            ))
                            .id(),
                    );
                    unit.rt_materials.push(copy);
                }
            }
            for entity in &unit.rt_entities {
                commands.entity(*entity).insert(if rt_ndc.is_some() { Visibility::Inherited } else { Visibility::Hidden });
            }
            if let (Some(m), true) = (rt_ndc, std::env::var_os("ROSES_BMODULE_LOG").is_some()) {
                if let Ok(frame) = unit.scene.frame() {
                    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                    for g in &frame {
                        for p in &g.positions {
                            let c = m * Vec4::new(p[0], p[1], p[2], 1.0);
                            // GS 512 window pixels from NDC.
                            let (x, y) = ((c.x / c.w + 1.0) * 256.0, (1.0 - c.y / c.w) * 256.0);
                            lo = [lo[0].min(x), lo[1].min(y)];
                            hi = [hi[0].max(x), hi[1].max(y)];
                        }
                    }
                    eprintln!("arena unit {} into its target with the callback's camera: x {:.1}..{:.1} y {:.1}..{:.1}", unit.role, lo[0], hi[0], lo[1], hi[1]);
                }
            }
            if let Some(m) = rt_ndc {
                for (copy, original) in unit.rt_materials.iter().zip(&unit.materials) {
                    let Some(base) = materials.get(original).cloned() else { continue };
                    if let Some(mut c) = materials.get_mut(copy) {
                        *c = base;
                        c.set_ndc(m);
                    }
                }
            }
            match unit.scene.frame() {
                Ok(frame) => {
                    for (handle, mesh) in unit.meshes.iter().zip(&frame) {
                        if let Some(mut target) = meshes.get_mut(handle) {
                            *target = mesh_of(mesh);
                        }
                    }
                }
                Err(error) => errors.push(format!("card {}: {error}", unit.card)),
            }
            for handle in &unit.materials {
                if let Some(mut material) = materials.get_mut(handle) {
                    material.set_ndc(ndc);
                }
            }
        }
        for error in errors {
            self.report(error);
        }
    }

    /// The free model (`None` removes it): built once per card, then posed,
    /// skinned and drawn every update on the board canvas with the board
    /// camera (normal depth, after the board as the units).
    #[allow(clippy::too_many_arguments)]
    pub fn sync_free(
        &mut self,
        commands: &mut Commands,
        pose: Option<FreePose>,
        view: &camera::Mat4,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<ModelMaterial>,
        mut created: Option<&mut Created>,
    ) {
        if self.free.as_ref().map(|u| u.card) != pose.map(|p| p.card) {
            if let Some(unit) = self.free.take() {
                for entity in unit.entities {
                    commands.entity(entity).despawn();
                }
            }
        }
        let Some(pose) = pose else { return };
        let ndc = to_mat4(&duel_board::ndc_from_source(view, &camera::identity()));
        if self.free.is_none() {
            let Some(loaded) = self.loaded(pose.card) else { return };
            let scene = match UnitScene::new(loaded, 0, (3, 3), 0, 1.0) {
                Ok(scene) => scene,
                Err(error) => {
                    self.report(format!("card {}: {error}", pose.card));
                    return;
                }
            };
            let dither = dither_mat4(&scene.loaded.dither);
            let mut unit = ArenaUnit {
                role: 0,
                card: pose.card,
                textures: ModelTextures::default(),
                entities: Vec::new(),
                meshes: Vec::new(),
                materials: Vec::new(),
                rt_entities: Vec::new(),
                rt_materials: Vec::new(),
                scene,
            };
            let frame = match unit.scene.frame() {
                Ok(frame) => frame,
                Err(error) => {
                    self.report(format!("card {}: {error}", pose.card));
                    return;
                }
            };
            for (group, mesh) in unit.scene.groups.iter().zip(&frame) {
                let mut new_textures = Vec::new();
                let texture = match unit.textures.get(images, &unit.scene.loaded, group, &mut new_textures) {
                    Ok(texture) => texture,
                    Err(error) => {
                        self.report(format!("card {}: {error}", pose.card));
                        return;
                    }
                };
                let mesh = meshes.add(mesh_of(mesh));
                let material = materials.add(model_material(&group.state, ndc, texture, dither, self.gs16));
                if let Some(c) = created.as_deref_mut() {
                    c.textures.extend(new_textures);
                    c.meshes.push(mesh.id());
                    c.materials.push(material.id().untyped());
                }
                unit.meshes.push(mesh);
                unit.materials.push(material);
            }
            let rank = self.active.len() + 1;
            for (order, (mesh, material)) in unit.meshes.iter().zip(&unit.materials).enumerate() {
                let depth = FIRST_GROUP_DEPTH + (rank * GROUPS_PER_UNIT + order) as f32;
                unit.entities.push(
                    commands
                        .spawn((
                            Name::new(format!("free model card {} group {order}", pose.card)),
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            Transform::from_xyz(0.0, 0.0, depth).with_scale(Vec3::splat(duel_models::DRAW_ORDER_SCALE)),
                            bevy::camera::visibility::NoFrustumCulling,
                            RenderLayers::layer(0),
                        ))
                        .id(),
                );
            }
            self.free = Some(unit);
        }
        let Some(unit) = self.free.as_mut() else { return };
        unit.scene.set_pose(pose.position, pose.yaw);
        unit.scene.set_scale(pose.scale);
        unit.scene.motion = pose.motion;
        unit.scene.clock = pose.clock;
        unit.scene.blend = pose.blend;
        let visible = pose.scale > 0.0;
        for entity in &unit.entities {
            commands.entity(*entity).insert(if visible { Visibility::Inherited } else { Visibility::Hidden });
        }
        let mut error = None;
        match unit.scene.frame() {
            Ok(frame) => {
                for (handle, mesh) in unit.meshes.iter().zip(&frame) {
                    if let Some(mut target) = meshes.get_mut(handle) {
                        *target = mesh_of(mesh);
                    }
                }
            }
            Err(e) => error = Some(format!("card {}: {e}", pose.card)),
        }
        for handle in &unit.materials {
            if let Some(mut material) = materials.get_mut(handle) {
                material.set_ndc(ndc);
            }
        }
        if let Some(e) = error {
            self.report(e);
        }
    }

    /// A headless update before the first frame: every wanted unit's clock
    /// will start this many updates later.
    pub fn count_update(&mut self, units: &[UnitInput]) {
        let wanted: Vec<UnitKey> = model_units(units).iter().map(|w| w.key).collect();
        self.pending.retain(|key, _| wanted.contains(key));
        for key in wanted {
            *self.pending.entry(key).or_insert(0) += 1;
        }
    }

    fn report(&mut self, what: String) {
        if self.reported.insert(what.clone()) {
            eprintln!("Duel board: unit model not drawn: {what}");
        }
    }

    /// The node 0 bounding-sphere radius of `card`'s model (object +0x1E4,
    /// the battle scene's unit camera distance), `None` without a model.
    pub fn radius(&mut self, card: u16) -> Option<f32> {
        if !self.cache.contains_key(&card) {
            self.loaded(card);
        }
        let model = self.cache.get(&card)?.as_ref()?;
        model.model.nodes.first().map(|n| n.bounding_sphere[3])
    }

    fn loaded(&mut self, card: u16) -> Option<LoadedModel> {
        if !self.cache.contains_key(&card) {
            let loaded = match duel_models::load_model(&self.manifest, card) {
                Ok(loaded) => Some(loaded),
                Err(error) => {
                    self.report(format!("card {card}: {error}"));
                    None
                }
            };
            self.cache.insert(card, loaded);
        }
        self.cache[&card].clone()
    }

    /// One update: units whose key changed are rebuilt, every model's
    /// motion clock advances (`tick`) and its meshes are re-skinned; the
    /// cursor unit moves to the unit canvas; materials follow the camera.
    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &mut self,
        commands: &mut Commands,
        units: &[UnitInput],
        cursor: (u8, u8),
        view: &camera::Mat4,
        tick: bool,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<ModelMaterial>,
        mut created: Option<&mut Created>,
    ) {
        let wanted = model_units(units);
        // Drop units that left or changed.
        let mut kept = Vec::new();
        for mut unit in std::mem::take(&mut self.active) {
            if let Some(w) = wanted.iter().find(|w| w.key == unit.key) {
                unit.cell = w.cell;
                unit.scene.set_place(w.cell, w.offset);
                unit.scene.set_turn(w.turn);
                if let Some(s) = w.scale {
                    unit.scene.set_scale(s);
                }
                kept.push(unit);
            } else {
                for entity in unit.entities {
                    commands.entity(entity).despawn();
                }
            }
        }
        self.active = kept;
        // The grow-in of models that appeared (a reveal or flip): scale
        // a / 128 with a += 8 per update (16 updates).
        if tick {
            for unit in &mut self.active {
                if let Some(a) = self.appear.get_mut(&unit.key) {
                    *a = (*a + 8).min(128);
                    unit.scene.set_scale(f32::from(*a) / 128.0);
                    if *a >= 128 {
                        self.appear.remove(&unit.key);
                    }
                }
            }
        }
        // Existing units advance one update (draws use the pre-update time
        // of the next frame); new ones start at SetMotion's frame.
        if tick {
            let mut errors = Vec::new();
            for unit in &mut self.active {
                unit.scene.tick();
                match unit.scene.frame() {
                    Ok(frame) => {
                        for (handle, mesh) in unit.meshes.iter().zip(&frame) {
                            if let Some(mut target) = meshes.get_mut(handle) {
                                *target = mesh_of(mesh);
                            }
                        }
                    }
                    Err(error) => errors.push(format!("card {}: {error}", unit.key.card)),
                }
            }
            for error in errors {
                self.report(error);
            }
        }
        let ndc = to_mat4(&duel_board::ndc_from_source(view, &camera::identity()));
        for want in &wanted {
            let key = &want.key;
            if self.active.iter().any(|u| u.key == *key) {
                continue;
            }
            let Some(loaded) = self.loaded(key.card) else {
                continue;
            };
            let scene = match UnitScene::new(loaded, 0, want.cell, key.side, 1.0) {
                Ok(mut scene) => {
                    for _ in 0..self.pending.remove(key).unwrap_or(0) {
                        scene.tick();
                    }
                    scene.set_place(want.cell, want.offset);
                    scene.set_turn(want.turn);
                    if self.seeded && key.object < 30 {
                        self.appear.insert(*key, 0);
                        scene.set_scale(0.0);
                    }
                    scene
                }
                Err(error) => {
                    self.report(format!("card {}: {error}", key.card));
                    continue;
                }
            };
            for message in &scene.skipped {
                self.report(format!("card {}: {message}", key.card));
            }
            let frame = match scene.frame() {
                Ok(frame) => frame,
                Err(error) => {
                    self.report(format!("card {}: {error}", key.card));
                    continue;
                }
            };
            let dither = dither_mat4(&scene.loaded.dither);
            let mut unit = ActiveUnit {
                key: *key,
                cell: want.cell,
                textures: ModelTextures::default(),
                entities: Vec::new(),
                meshes: Vec::new(),
                materials: Vec::new(),
                cursor: false,
                scene,
            };
            if unit.scene.groups.len() > GROUPS_PER_UNIT {
                self.report(format!(
                    "card {}: {} draw groups exceed {GROUPS_PER_UNIT}",
                    key.card,
                    unit.scene.groups.len()
                ));
                continue;
            }
            let mut failed = None;
            for (group, mesh) in unit.scene.groups.iter().zip(&frame) {
                let mut new_textures = Vec::new();
                let texture =
                    match unit
                        .textures
                        .get(images, &unit.scene.loaded, group, &mut new_textures)
                    {
                        Ok(texture) => texture,
                        Err(error) => {
                            failed = Some(error);
                            break;
                        }
                    };
                let mesh = meshes.add(mesh_of(mesh));
                let material = materials.add(model_material(
                    &group.state,
                    ndc,
                    texture,
                    dither,
                    self.gs16,
                ));
                if let Some(c) = created.as_deref_mut() {
                    c.textures.extend(new_textures);
                    c.meshes.push(mesh.id());
                    c.materials.push(material.id().untyped());
                }
                unit.meshes.push(mesh);
                unit.materials.push(material);
            }
            if let Some(error) = failed {
                self.report(format!("card {}: {error}", key.card));
                continue;
            }
            self.active.push(unit);
        }
        self.active.sort_by_key(|u| u.key);
        // Entities: (re)spawned when a unit is new or its layer changes.
        for (rank, unit) in self.active.iter_mut().enumerate() {
            let cursor_unit = unit.cell == cursor;
            if !unit.entities.is_empty() && unit.cursor == cursor_unit {
                continue;
            }
            for entity in unit.entities.drain(..) {
                commands.entity(entity).despawn();
            }
            unit.cursor = cursor_unit;
            let layer = if cursor_unit { UNIT_LAYER } else { 0 };
            for (order, (mesh, material)) in unit.meshes.iter().zip(&unit.materials).enumerate() {
                let depth = FIRST_GROUP_DEPTH + (rank * GROUPS_PER_UNIT + order) as f32;
                let entity = commands
                    .spawn((
                        Name::new(format!(
                            "unit object {} card {} group {order}",
                            unit.key.object, unit.key.card
                        )),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_xyz(0.0, 0.0, depth)
                            .with_scale(Vec3::splat(duel_models::DRAW_ORDER_SCALE)),
                        bevy::camera::visibility::NoFrustumCulling,
                        RenderLayers::layer(layer),
                    ))
                    .id();
                unit.entities.push(entity);
            }
        }
        // The near-camera fade: hidden at 0, drawn alone on the fade canvas
        // and composited between 0 and 128.
        self.levels = wanted.iter().map(|w| (w.key, w.level)).collect();
        for unit in &self.active {
            let alpha = self.unit_alpha(unit.key);
            let layer = if alpha < 128 {
                FADE_LAYER
            } else if unit.cursor {
                UNIT_LAYER
            } else {
                0
            };
            for entity in &unit.entities {
                commands.entity(*entity).insert((
                    if alpha > 0 { Visibility::Inherited } else { Visibility::Hidden },
                    RenderLayers::layer(layer),
                ));
            }
        }
        if self.applied_view.as_ref() != Some(view) {
            for unit in &self.active {
                for handle in &unit.materials {
                    if let Some(mut material) = materials.get_mut(handle) {
                        material.set_ndc(ndc);
                    }
                }
            }
            self.applied_view = Some(*view);
        }
        self.seeded = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(side: u8, slot: u8, card: u16, face_up: bool) -> UnitInput {
        UnitInput {
            side,
            slot,
            cell: [slot, side],
            card,
            face_up,
            defence: false,
            acted: false,
            outline_pulse: false,
            face: None,
            offset: [0.0; 2],
            turn: 0,
            flip: None,
            alpha: None,
            sideways: None,
            spin: None,
            model_scale: None,
            emblem: None,
        }
    }

    #[test]
    fn leaders_and_face_up_monsters_have_models_in_object_order() {
        let units = [
            unit(1, 0, 397, false),
            unit(0, 0, 392, false),
            unit(0, 2, 10, true),
            unit(0, 3, 11, false),
            unit(1, 6, 300, true),
            unit(1, 5, 12, true),
        ];
        let keys: Vec<(u8, u16)> = model_units(&units)
            .iter()
            .map(|w| (w.key.object, w.key.card))
            .collect();
        assert_eq!(keys, [(21, 10), (29, 12), (30, 392), (31, 397)]);
    }
}
