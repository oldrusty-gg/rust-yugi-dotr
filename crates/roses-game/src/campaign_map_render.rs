//! `--campaign-map <manifest>`: the campaign map (Lancastrian route) from a
//! private `roses import-campaign-map` import, drawn live with the traced map
//! camera and driven by `roses_client::campaign_map::MapModel` at 25 Hz.
//! Scene rules live in the GPU-free `campaign_map` module; this file only
//! drives Bevy.
//!
//! The map renders into a 512x512 Rgba8Unorm canvas (GS byte arithmetic,
//! fixed-function blending for PRIM ABE strips); a present pass composites a
//! placeholder 2D layer (imported message images at explicit placeholder
//! positions, a crosshair for the cursor) and shows the result at 4:3 or
//! captures it. Arrows move the cursor (held), X/Enter is Cross, Escape
//! closes. Cross on Chester starts the taunt pages and the walk; the mode
//! stops at the explicit "duel against Weevil would start here" boundary
//! (window title and stderr). `--map-result win|loss` starts at the return
//! from that duel instead.
//!
//! Explicitly not implemented: HUD, hover panel, portraits and window art
//! (placeholder text images only), the leader figure model (a plain marker),
//! the ship's routes, the sea pieces' motion, the □ scroll mode and SYSTEM
//! icons, the DC check, sound.
use super::{
    CaptureBridge, CaptureState, CaptureWatchdog, PresentationCamera, drive_capture,
    exit_on_escape, fit_view, inspect_render_progress,
};
use crate::cloud_render::CloudMaterial;
use crate::duel_board_render;
use bevy::{
    app::ScheduleRunnerPlugin,
    asset::RenderAssetUsages,
    camera::{
        RenderTarget,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{
        CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageSampler,
        ImageSamplerDescriptor, ImageType,
    },
    input::{
        ButtonState,
        keyboard::{Key, KeyboardFocusLost, KeyboardInput},
    },
    mesh::{Indices, MeshVertexBufferLayoutRef},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
            Extent3d, PrimitiveTopology, RenderPipelineDescriptor, SpecializedMeshPipelineError,
            TextureDimension, TextureFormat, TextureUsages,
        },
    },
    shader::ShaderRef,
    window::{ExitCondition, PrimaryWindow},
    winit::WinitPlugin,
};
use roses_game::camera;
use roses_game::campaign_map::{
    self as scene, Blend, CURSOR_HALF, DrawMesh, Instance, LayerItem, LoadedMap,
};
use roses_game::campaign_map_hud as hud;
use roses_client::campaign_map::{self as map, MapEvent, MapModel as MapSim, Pad};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};

const PRESENT_PIPELINE: &str = "roses-duel-board-present";
const BOUNDARY: &str = "duel against the hovered opponent would start here (the duel mode runs separately: --duel-board ... --duel-play)";
const MAX_TICKS: u32 = 100_000;
const CANVAS: u32 = 512;
/// The story overlay's canvas (`roses_client::Loaded::render_overlay`).
const STORY_CANVAS: [u32; 2] = [640, 448];
const FIGURE_LABEL: &str = "leader figure (placeholder marker)";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(MapKey)]
pub struct MapMaterial {
    #[uniform(0)]
    ndc_from_source: Mat4,
    #[uniform(1)]
    params: Vec4,
    #[texture(2)]
    #[sampler(3)]
    texture: Handle<Image>,
    #[uniform(4)]
    dither_rows: Mat4,
    #[uniform(5)]
    extra: Vec4,
    /// Height fog (battle arena stage): (Q0, Q1, 1 = on, 0), FOGCOL bytes,
    /// and the model-to-world Y row (Y = row.xyz . position + row.w).
    #[uniform(6)]
    fog: Vec4,
    #[uniform(7)]
    fog_colour: Vec4,
    #[uniform(8)]
    world_y: Vec4,
    blend: u8,
    /// ZMSK 1: no Z write (the battle arena's blended stage draws).
    z_masked: bool,
}
#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct MapKey {
    blend: u8,
    z_masked: bool,
}
impl From<&MapMaterial> for MapKey {
    fn from(material: &MapMaterial) -> Self {
        Self {
            blend: material.blend,
            z_masked: material.z_masked,
        }
    }
}
impl MapMaterial {
    /// A map-grammar draw's material (the battle arena reuses the map's
    /// stage draws): texture use, ATST, TCC and blend from the draw.
    pub(crate) fn for_draw(draw: &DrawMesh, texture: Handle<Image>, ndc: Mat4) -> Self {
        Self {
            ndc_from_source: ndc,
            params: Vec4::new(
                f32::from(u8::from(draw.textured)),
                draw.alpha_greater_than.map_or(-1.0, f32::from),
                0.0,
                f32::from(u8::from(draw.texture_alpha)),
            ),
            texture,
            dither_rows: Mat4::ZERO,
            extra: Vec4::new(1.0, 0.0, 0.0, 0.0),
            fog: Vec4::ZERO,
            fog_colour: Vec4::ZERO,
            world_y: Vec4::ZERO,
            blend: match draw.blend {
                Blend::Opaque => 0,
                Blend::Standard => 1,
                Blend::Additive => 2,
            },
            z_masked: false,
        }
    }
    /// The battle arena's blended stage draws write no Z (ZMSK 1) [emu
    /// dumps-bmodule-043 / card373-c240: every ABE stage draw].
    pub(crate) fn set_z_masked(&mut self, masked: bool) {
        self.z_masked = masked;
    }
    pub(crate) fn set_ndc(&mut self, ndc: Mat4) {
        self.ndc_from_source = ndc;
    }
    /// The arena stage light g (0x2DF5B2, 128 = 1.0).
    pub(crate) fn stage_light(&self) -> u8 {
        (128.0 - self.extra.z * 128.0).round() as u8
    }
    pub(crate) fn set_stage_light(&mut self, g: u8) {
        self.extra.z = 1.0 - f32::from(g) / 128.0;
    }
    /// The arena's height fog with the model's world Y row.
    pub(crate) fn set_fog(&mut self, fog: [f32; 4], colour: [f32; 4], world_y: [f32; 4]) {
        self.fog = Vec4::from_array(fog);
        self.fog_colour = Vec4::from_array(colour);
        self.world_y = Vec4::from_array(world_y);
    }
}
impl Material for MapMaterial {
    fn vertex_shader() -> ShaderRef {
        "campaign_map.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "campaign_map.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        // The sorted phase keeps the original draw order (see `order`).
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The capture bridge waits for the board pipeline label; the map draws
        // use the same shader structure and share it.
        descriptor.label = Some(duel_board_render::PIPELINE.into());
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?];
        descriptor.primitive.cull_mode = None;
        let keep = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        let color = match key.bind_group_data.blend {
            // ALPHA 0x44: (Cs - Cd) * As + Cd.
            1 => Some(BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            }),
            // ALPHA 0x48: Cs * As + Cd.
            2 => Some(BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            }),
            _ => None,
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = color.map(|color| BlendState { color, alpha: keep });
            }
        }
        if let Some(depth) = &mut descriptor.depth_stencil {
            // TEST ZTE 1, ZTST GREATER, Z writes on for every map draw (off
            // for the arena's blended stage draws).
            depth.depth_compare = Some(CompareFunction::Greater);
            depth.depth_write_enabled = Some(!key.bind_group_data.z_masked);
        }
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PresentMaterial {
    #[texture(0)]
    #[sampler(1)]
    canvas: Handle<Image>,
    #[uniform(2)]
    params: Vec4,
    #[texture(3)]
    #[sampler(4)]
    layer: Handle<Image>,
    /// The shared shader's duel unit canvas, tint and dither: a transparent
    /// 1x1 image (no unit layer on the map).
    #[texture(5)]
    #[sampler(6)]
    units: Handle<Image>,
    #[uniform(7)]
    unit_tint: Vec4,
    #[uniform(8)]
    dither_rows: Mat4,
    /// The shared shader's versus canvas (unused: params.w = 0).
    #[texture(9)]
    #[sampler(10)]
    versus: Handle<Image>,
    /// The shared shader's blur history (unused: blur.x = 0).
    #[texture(11)]
    #[sampler(12)]
    history: Handle<Image>,
    #[uniform(13)]
    blur: Vec4,
    /// The shared shader's over-HUD effects canvas (black: adds nothing).
    #[texture(14)]
    #[sampler(15)]
    top: Handle<Image>,
    /// The shared shader's cover canvas (transparent: changes nothing).
    #[texture(16)]
    #[sampler(17)]
    cover: Handle<Image>,
}
impl Material for PresentMaterial {
    fn vertex_shader() -> ShaderRef {
        "duel_board_present.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "duel_board_present.wgsl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.label = Some(PRESENT_PIPELINE.into());
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
        ])?];
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// One map draw entity: its instance label, last applied world and alpha.
#[derive(Component)]
struct MapDrawEntity {
    label: String,
    world: camera::Mat4,
    alpha: f32,
}

#[derive(Resource)]
struct MapInput {
    loaded: LoadedMap,
}

#[derive(Resource)]
struct MapRuntime {
    sim: MapSim,
    keys: MapKeys,
    title: String,
    trace: bool,
    applied_target: Option<[f64; 2]>,
    layer_items: Vec<LayerItem>,
    layer_dirty: bool,
    /// The original 2D HUD (0x21D4E0's panels, cursor and tooltip) when the
    /// game supplied the duel import; otherwise placeholders.
    hud: Option<MapHud>,
    /// SYSTEM → SAVE's memory-card dialog (0x1AB8F0(1) / 0x1ABB20 while
    /// W+543E = 1) and the card it writes.
    card: Option<(roses_client::memory_card::CardDialog, roses_game::memory_card_store::VirtualCard)>,
    /// OPTION written this session (taken by the game: the player's block
    /// and the mixer).
    options_saved: Option<u8>,
    /// The model ran an update this FixedUpdate (not held for a capture)
    /// and its pad's Cross: a story played over the map (`MapStoryHost`)
    /// moves with it and takes that Cross.
    stepped: bool,
    story_cross: bool,
}

/// The inputs of the map's 2D HUD (the duel import's executable, message
/// file, TEXSYS.MRG and PICMINI.MRG, and the player's deck).
pub struct MapHudSource {
    pub executable: Vec<u8>,
    pub messages: Vec<u8>,
    pub texsys: Vec<u8>,
    pub picmini: Vec<u8>,
    pub deck: hud::DeckHud,
    /// Each opponent's static deck leader and rank (record 0x1A + C4).
    pub opponents: BTreeMap<u8, hud::OpponentHud>,
    /// The map import's TEXANM.MRG (talking faces), when present.
    pub texanm: Option<Vec<u8>>,
    /// The models import's manifest (MONSTER.MRG): the leader figure.
    pub models_manifest: Option<std::path::PathBuf>,
    /// The player's name (SAVE 0x299214, twelve words, end bit 0x8000 on
    /// the last character): message 4's twelve name slots.
    pub player_name: Vec<u16>,
}

struct MapHud {
    executable: Vec<u8>,
    tables: roses_client::duel_hud::HudTables,
    textures: roses_client::duel_hud::HudTextures,
    deck: hud::DeckHud,
    opponents: BTreeMap<u8, hud::OpponentHud>,
    calls: Vec<hud::Call>,
    rgba: Vec<u8>,
    reported: std::collections::BTreeSet<String>,
    /// The talking faces (0x247310 slots): tables, the user's TEXANM.MRG,
    /// the panel's face, its decoded texture and the rand stream.
    faces: Option<roses_client::face_anim::FaceTables>,
    texanm: Option<Vec<u8>>,
    face: Option<roses_client::face_anim::Face>,
    face_texture: Option<(u8, roses_client::gs::Texture)>,
    face_draws: Vec<roses_client::face_anim::FaceDraw>,
    rand: roses_client::face_anim::NewlibRand,
    /// The memory-card dialog's box (overlay slot 3, drawn after the HUD
    /// by 0x1B0120) and the press icon's counter; `card_drawn` = last drawn.
    card_box: Option<(roses_client::memory_card::CardBox, u32)>,
    card_drawn: Option<(roses_client::memory_card::CardBox, u32)>,
    yes_no: Option<roses_client::duel_hud::YesNoTables>,
    /// The system dialogs last drawn (Select's YES/NO box, the OPTION panel).
    system_drawn: Option<SystemOverlay>,
}

/// What the system dialogs show this update (overlay slots 3 and 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SystemOverlay {
    title: Option<(i16, bool)>,
    options: Option<roses_client::duel_hud::OptionPanel>,
}

fn system_overlay(sim: &MapSim) -> Option<SystemOverlay> {
    let title = sim.title_dialog.as_ref().map(|d| (d.y(), d.yes));
    let options = sim.options.as_ref().map(|p| roses_client::duel_hud::OptionPanel {
        side: p.side,
        y: p.y(),
        row: p.row,
        bits: p.bits,
    });
    (title.is_some() || options.is_some()).then_some(SystemOverlay { title, options })
}

impl MapHud {
    fn new(source: MapHudSource) -> Result<Self, String> {
        use roses_client::duel_hud::{HudTables, HudTextures};
        let mut ids = HudTables::required_messages();
        ids.push(4);
        ids.extend(198..=260);
        ids.push(map::CAPTION_MESSAGE);
        // The map's page texts (encounter, win/loss; the White Rose side's
        // 3103..=3383 with the captions 3180 and 3300).
        ids.extend(2400..=3400);
        // The memory-card dialogs' messages (SYSTEM → SAVE).
        ids.extend(roses_client::memory_card::msg::ALL);
        // The OPTION panel's rows and "Return to Title Screen?".
        ids.extend(115..=123);
        ids.push(map::RETURN_TO_TITLE_MESSAGE);
        // SYSTEM icon 3's lesson menu rows (MSG 4087..=4091).
        ids.extend(map::TUTORIAL_LABEL_BASE..map::TUTORIAL_LABEL_BASE + u16::from(map::TUTORIAL_ROWS));
        ids.sort_unstable();
        ids.dedup();
        let mut codes = BTreeMap::new();
        for chunk in ids.chunks(256) {
            let parsed = roses_assets::parse_pal_messages(&source.messages, chunk)
                .map_err(|e| e.to_string())?;
            for m in parsed.messages {
                codes.insert(m.message_id, m.expanded_codes);
            }
        }
        // Message 4 is twelve player-name slots (0x2002); the compose stops
        // at the name's end bit.
        let end = source
            .player_name
            .iter()
            .position(|c| c & 0x8000 != 0)
            .map_or(source.player_name.len(), |i| i + 1);
        let name: Vec<u16> = source.player_name[..end].iter().map(|c| c & 0x7FFF).collect();
        if !name.is_empty() {
            codes.insert(4, name);
        }
        let tables = HudTables::from_executable(&source.executable, codes)?;
        let faces = match roses_client::face_anim::FaceTables::from_executable(&source.executable) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("Campaign map HUD: the face tables are unavailable ({e})");
                None
            }
        };
        let leader = source.deck.leader;
        let textures =
            HudTextures::new(&source.texsys, &source.picmini, [leader, leader], &source.messages)?;
        let yes_no = roses_client::duel_hud::YesNoTables::from_executable(&source.executable)
            .map_err(|e| eprintln!("Campaign map HUD: the YES/NO strings are unavailable ({e})"))
            .ok();
        Ok(Self {
            executable: source.executable,
            tables,
            textures,
            deck: source.deck,
            opponents: source.opponents,
            calls: Vec::new(),
            rgba: Vec::new(),
            reported: Default::default(),
            faces,
            texanm: source.texanm,
            face: None,
            face_texture: None,
            face_draws: Vec::new(),
            // [H] The rand state at map entry is not reproduced.
            rand: roses_client::face_anim::NewlibRand(1),
            card_box: None,
            card_drawn: None,
            yes_no,
            system_drawn: None,
        })
    }

    /// Request, release and step the panel's face (0x2356B8 / 0x2474B0 /
    /// 0x247A10 once per update); true if its sprites changed.
    fn update_face(&mut self, sim: &MapSim) -> bool {
        let before = std::mem::take(&mut self.face_draws);
        let (Some(tables), Some(texanm)) = (&self.faces, &self.texanm) else {
            return !before.is_empty();
        };
        // Mako's dialogue panel (12040/12041) has its own face slot; the
        // hover panel is off screen meanwhile.
        let request = hud::dialogue_face(sim).or(hud::panel_face(sim).map(|f| (f, 0)));
        match request {
            None => self.face = None,
            Some(((speaker, kind, x, y), mood)) => {
                let expression = tables.expression(speaker, mood).unwrap_or(0);
                match &mut self.face {
                    Some(f) if f.speaker == speaker => f.request(expression, kind, x, y),
                    _ => self.face = tables.face(speaker, expression, kind, x, y).ok(),
                }
                if self.face_texture.as_ref().map(|t| t.0) != Some(speaker) {
                    self.face_texture = roses_assets::decode_pal_opening_indexed_texture(
                        texanm,
                        roses_assets::OpeningArchiveKind::TexAnm,
                        usize::from(speaker),
                    )
                    .ok()
                    .map(|t| {
                        (
                            speaker,
                            roses_client::gs::Texture::from_c16_clut(
                                t.width as usize,
                                t.height as usize,
                                &t.palette_indices,
                                &t.palette_words_raw,
                                0,
                                0x80,
                            ),
                        )
                    });
                }
            }
        }
        if let Some(face) = &mut self.face {
            let rand = &mut self.rand;
            face.step(&mut || rand.next());
            self.face_draws = face.draws();
        }
        self.face_draws != before
    }

    /// The story windows' messages (11120/11124/11149) and Mako's pages
    /// (12040/12041) for map imports made before they were listed: line counts and widths from the user's
    /// message file (the page reveal and the window size read them).
    fn complete_messages(&self, sim: &mut MapSim) {
        for id in map::map_story_message_ids() {
            if sim.data.messages.contains_key(&id) {
                continue;
            }
            if let Some(codes) = self.tables.message(id) {
                sim.data
                    .messages
                    .insert(id, roses_client::campaign_map::MessageInfo::from_codes(codes));
            }
        }
    }

    /// Rebuild the layer when the HUD's calls changed; true if they did.
    fn update(&mut self, sim: &MapSim) -> bool {
        let face_changed = self.update_face(sim);
        let (calls, mut notes) = hud::calls(sim, &self.executable, &self.deck, &self.opponents);
        let system = system_overlay(sim);
        if calls == self.calls
            && !self.rgba.is_empty()
            && !face_changed
            && self.card_box == self.card_drawn
            && system == self.system_drawn
        {
            return false;
        }
        let mut prims = hud::primitives(&calls, &self.tables, &mut notes);
        if let (Some((b, counter)), Some(yes_no)) = (self.card_box, self.yes_no.as_ref()) {
            let frame = roses_client::duel_hud::overlay_box(
                &self.tables,
                yes_no,
                &roses_game::memory_card_store::overlay(b),
                counter,
            );
            notes.extend(frame.unsupported.iter().map(|u| hud::Note(format!("memory-card box: {:?}", u.kind))));
            prims.extend(frame.prims);
        }
        self.card_drawn = self.card_box;
        // 0x1B0120 draws slot 0 (the OPTION panel) before slot 3 (the box).
        if let (Some(o), Some(yes_no)) = (system, self.yes_no.as_ref()) {
            if let Some(panel) = o.options {
                let frame = roses_client::duel_hud::option_panel(&self.tables, yes_no, &panel);
                notes.extend(frame.unsupported.iter().map(|u| hud::Note(format!("OPTION panel: {:?}", u.kind))));
                prims.extend(frame.prims);
            }
            if let Some((y, yes)) = o.title {
                let b = roses_client::duel_hud::OverlayBox {
                    kind: roses_client::duel_hud::OverlayKind::YesNo { yes },
                    x: 0,
                    y,
                    text: map::RETURN_TO_TITLE_MESSAGE,
                };
                let frame = roses_client::duel_hud::overlay_box(&self.tables, yes_no, &b, 0);
                notes.extend(frame.unsupported.iter().map(|u| hud::Note(format!("title box: {:?}", u.kind))));
                prims.extend(frame.prims);
            }
        }
        self.system_drawn = system;
        let (rgba, skipped) = roses_client::duel_hud::render_primitives_layer(
            &prims,
            &self.tables,
            &mut self.textures,
            CANVAS as usize,
            CANVAS as usize,
        );
        for text in notes
            .into_iter()
            .map(|n| n.0)
            .chain(skipped.into_iter().map(|s| format!("primitive {} skipped: {}", s.index, s.reason)))
        {
            if self.reported.insert(text.clone()) {
                eprintln!("Campaign map HUD: {text}");
            }
        }
        self.calls = calls;
        self.rgba = rgba;
        if let Some((_, texture)) = &self.face_texture {
            for d in &self.face_draws {
                draw_face_sprite(&mut self.rgba, texture, d);
            }
        }
        true
    }
}

/// One face sprite (0x2474E0) over the premultiplied HUD layer: bilinear
/// (0x207E50(.., 1)) inside the CLAMP region, ALPHA 0x44 with the texel's
/// alpha (0 or 0x80 through TEXA). [H] The sample position is the pixel
/// centre mapped linearly across the rect.
fn draw_face_sprite(
    layer: &mut [u8],
    texture: &roses_client::gs::Texture,
    d: &roses_client::face_anim::FaceDraw,
) {
    let [u0, v0, u1, v1] = d.tex_rect.map(f32::from);
    let [x0, y0, x1, y1] = d.dest_rect;
    if x1 <= x0 || y1 <= y0 || u1 <= u0 || v1 <= v0 {
        return;
    }
    let (su, sv) = ((u1 - u0) / (x1 - x0) as f32, (v1 - v0) / (y1 - y0) as f32);
    let texel = |u: i32, v: i32| -> [f32; 4] {
        let u = u.clamp(u0 as i32, u1 as i32 - 1).clamp(0, texture.width as i32 - 1) as usize;
        let v = v.clamp(v0 as i32, v1 as i32 - 1).clamp(0, texture.height as i32 - 1) as usize;
        texture.texels[v * texture.width + u].map(f32::from)
    };
    for py in y0.max(0)..y1.min(CANVAS as i32) {
        for px in x0.max(0)..x1.min(CANVAS as i32) {
            let u = u0 + (px - x0) as f32 * su + su / 2.0 - 0.5;
            let v = v0 + (py - y0) as f32 * sv + sv / 2.0 - 0.5;
            let (fu, fv) = (u.floor(), v.floor());
            let (au, av) = (u - fu, v - fv);
            let (iu, iv) = (fu as i32, fv as i32);
            let (a, b, c, e) = (texel(iu, iv), texel(iu + 1, iv), texel(iu, iv + 1), texel(iu + 1, iv + 1));
            let mut s = [0f32; 4];
            for k in 0..4 {
                s[k] = (a[k] * (1.0 - au) + b[k] * au) * (1.0 - av) + (c[k] * (1.0 - au) + e[k] * au) * av;
            }
            let alpha = (s[3] / 128.0).clamp(0.0, 1.0);
            if alpha <= 0.0 {
                continue;
            }
            let at = ((py as u32 * CANVAS + px as u32) * 4) as usize;
            for k in 0..3 {
                let dst = f32::from(layer[at + k]);
                layer[at + k] = (s[k] * alpha + dst * (1.0 - alpha)).round().clamp(0.0, 255.0) as u8;
            }
            let da = f32::from(layer[at + 3]);
            layer[at + 3] = (255.0 * alpha + da * (1.0 - alpha)).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// The placeholder items the HUD replaces (cursor, tooltip, over-DC text,
/// the story window's text and the letterbox, which the HUD draws in the
/// original's order: letterbox, then the story window).
fn without_hud_items(items: Vec<LayerItem>, hud: bool) -> Vec<LayerItem> {
    if !hud {
        return items;
    }
    items
        .into_iter()
        .filter(|i| {
            !matches!(
                i,
                LayerItem::Cursor { .. }
                    | LayerItem::Tooltip { .. }
                    | LayerItem::CenteredText { .. }
                    | LayerItem::Letterbox { .. }
            ) && !matches!(i, LayerItem::Text { message, .. } if *message != map::ROSE_CARD_MESSAGE)
        })
        .collect()
}

/// Host key events arrive between 25 Hz samples. Preserve a brief press even
/// if its release arrives in the same batch; holding a direction still feeds
/// every sample, while Cross is emitted only once per fresh press. The
/// dialogs' Up/Down repeat is the pad driver's (0x203380 mode 2: first
/// repeat 13 updates after the press, then every update), from the held
/// word — never the host's key repeat.
struct MapKeys {
    held: BTreeMap<KeyCode, Pad>,
    pressed: Pad,
    repeat: roses_game::duel_play::PadRepeat,
    /// New presses for the memory-card dialog (X/Enter Cross, Z Circle,
    /// A Square, D Triangle, Left/Right), as in the other modes.
    card: roses_client::memory_card::CardPad,
}

impl Default for MapKeys {
    fn default() -> Self {
        Self {
            held: BTreeMap::new(),
            pressed: Pad::NONE,
            repeat: Default::default(),
            card: Default::default(),
        }
    }
}

impl MapKeys {
    fn event(&mut self, event: &KeyboardInput) {
        match event.state {
            ButtonState::Pressed if !event.repeat => {
                match event.key_code {
                    KeyCode::KeyX | KeyCode::Enter => self.card.cross = true,
                    KeyCode::KeyZ => self.card.circle = true,
                    KeyCode::KeyA => self.card.square = true,
                    KeyCode::KeyD => self.card.triangle = true,
                    KeyCode::ArrowLeft => self.card.left = true,
                    KeyCode::ArrowRight => self.card.right = true,
                    _ => {}
                }
                // The system dialogs' presses: Tab Select, Z Circle, A
                // Square, Q L1, E R1 (as in the duel).
                match event.key_code {
                    KeyCode::Tab => self.pressed.menu.select = true,
                    KeyCode::KeyZ => self.pressed.menu.circle = true,
                    KeyCode::KeyA => self.pressed.menu.square = true,
                    KeyCode::KeyQ => self.pressed.menu.l1 = true,
                    KeyCode::KeyE => self.pressed.menu.r1 = true,
                    _ => {}
                }
                let pad = key_pad(event);
                if pad != Pad::NONE {
                    self.pressed.up |= pad.up;
                    self.pressed.down |= pad.down;
                    self.pressed.left |= pad.left;
                    self.pressed.right |= pad.right;
                    self.pressed.cross |= pad.cross;
                    self.pressed.cross_held |= pad.cross;
                    self.held.insert(event.key_code, pad);
                }
            }
            // The host's key repeat: ignored (the pad's own counters repeat).
            ButtonState::Pressed => {}
            ButtonState::Released => {
                self.held.remove(&event.key_code);
            }
        }
    }

    fn sample(&mut self) -> Pad {
        let pad = self.held.values().fold(self.pressed, |acc, held| Pad {
            up: acc.up || held.up,
            down: acc.down || held.down,
            left: acc.left || held.left,
            right: acc.right || held.right,
            cross: acc.cross,
            cross_held: acc.cross_held || held.cross,
            menu: acc.menu,
        });
        let mut pad = pad;
        // New presses of the directions and Cross count for the dialogs too.
        pad.menu.cross |= self.pressed.cross;
        pad.menu.left |= self.pressed.left;
        pad.menu.right |= self.pressed.right;
        pad.menu.up |= self.pressed.up;
        pad.menu.down |= self.pressed.down;
        // Up/Down repeat (0x203380 mode 2) for the dialogs' rows.
        use roses_game::pads::bit;
        let word = (if pad.up { bit::UP } else { 0 }) | (if pad.down { bit::DOWN } else { 0 });
        let (_, repeats) = self.repeat.frame_split(word);
        pad.menu.up |= repeats & bit::UP != 0;
        pad.menu.down |= repeats & bit::DOWN != 0;
        self.pressed = Pad::NONE;
        pad
    }

    fn lose_focus(&mut self) {
        self.held.clear();
        self.pressed = Pad::NONE;
        self.card = Default::default();
    }

    fn take_card(&mut self) -> roses_client::memory_card::CardPad {
        std::mem::take(&mut self.card)
    }
}

/// The 2D layer image (premultiplied RGBA bytes over the raw 512 canvas).
#[derive(Resource)]
struct LayerImage {
    image: Option<Handle<Image>>,
    present: Option<Handle<PresentMaterial>>,
    /// The present pass's cover canvas (`MapStoryHost`'s overlay).
    story: Option<Handle<Image>>,
    /// Decoded message images by id: (rgba, width, height).
    decoded: BTreeMap<u16, (Vec<u8>, u32, u32)>,
}

struct Options {
    manifest: PathBuf,
    result: Option<bool>,
    replay: Vec<Pad>,
    ticks: Option<u32>,
    capture: Option<PathBuf>,
    validate_only: bool,
}

fn parse(args: impl Iterator<Item = OsString>) -> Result<Options, String> {
    let mut options = Options {
        manifest: PathBuf::new(),
        result: None,
        replay: Vec::new(),
        ticks: None,
        capture: None,
        validate_only: false,
    };
    let mut manifest = None;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--campaign-map") if manifest.is_none() => {
                manifest = Some(PathBuf::from(
                    args.next()
                        .ok_or("--campaign-map requires a manifest path")?,
                ));
            }
            Some("--map-result") => {
                options.result = match args.next().as_ref().and_then(|v| v.to_str()) {
                    Some("win") => Some(true),
                    Some("loss") => Some(false),
                    _ => return Err("--map-result must be win or loss".into()),
                };
            }
            Some("--replay") => {
                let text = args
                    .next()
                    .ok_or("--replay requires comma-separated pads")?
                    .into_string()
                    .map_err(|_| "--replay must be UTF-8")?;
                options.replay = scene::parse_replay(&text)?;
            }
            Some("--ticks") => {
                let text = args
                    .next()
                    .ok_or("--ticks requires a count")?
                    .into_string()
                    .map_err(|_| "--ticks must be UTF-8")?;
                let ticks: u32 = text
                    .parse()
                    .map_err(|_| format!("Invalid --ticks: {text}"))?;
                if ticks > MAX_TICKS {
                    return Err(format!("--ticks accepts at most {MAX_TICKS} updates"));
                }
                options.ticks = Some(ticks);
            }
            Some("--validate-only") => options.validate_only = true,
            Some("--capture") if options.capture.is_none() => {
                options.capture = Some(PathBuf::from(
                    args.next().ok_or("--capture requires a new PNG path")?,
                ));
            }
            Some("--help") => {
                println!(
                    "roses-bevy-scene --campaign-map <campaign-map manifest.json> [--map-result win|loss] [--replay up*5,cross,none*30,... [--ticks n]] [--validate-only | --capture <new.png>]\nThe map at event 0 (after landing at Milford Haven): arrows move the cursor (held), X or Enter is Cross, Escape closes. Cross on an open node (Chester) shows its taunt pages, walks the leader marker along the path and stops at the explicit duel boundary. --map-result starts at the return from the Chester duel instead (win: text, rose card, reveal of Lancashire and Towton; loss: text, node lost). --replay applies one pad per 25 Hz update from the start, then runs --ticks updates in total (default: entries) before the first frame. Placeholder text images and a crosshair stand in for the untraced HUD, panels and cursor art."
                );
                std::process::exit(0);
            }
            Some("--faithful") => {}
            _ => {
                return Err(format!(
                    "Unknown campaign-map argument: {}",
                    arg.to_string_lossy()
                ));
            }
        }
    }
    options.manifest = manifest.ok_or("--campaign-map requires a manifest path")?;
    if options
        .ticks
        .is_some_and(|t| (t as usize) < options.replay.len())
    {
        return Err("--ticks must cover every --replay update".into());
    }
    if options.validate_only && options.capture.is_some() {
        return Err("--validate-only and --capture cannot be combined".into());
    }
    if let Some(path) = &options.capture {
        if path
            .extension()
            .and_then(|v| v.to_str())
            .is_none_or(|v| !v.eq_ignore_ascii_case("png"))
        {
            return Err("Capture output must have a .png extension".into());
        }
        if path.exists() {
            return Err(format!("Capture output already exists: {}", path.display()));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        if !parent.is_dir() {
            return Err(format!(
                "Capture output directory does not exist: {}",
                parent.display()
            ));
        }
    }
    Ok(options)
}

fn title(sim: &MapSim) -> String {
    let mut caption = format!("Roses Bevy campaign map · {}", sim.caption());
    if sim.duel_request.is_some() {
        caption.push_str(" · BOUNDARY: ");
        caption.push_str(BOUNDARY);
    }
    for note in &sim.notes {
        caption.push_str(" · ");
        caption.push_str(note);
    }
    caption.push_str(" · placeholders: leader figure, rose-card animation (the HUD is drawn from the duel import when present)");
    caption
}

/// Held pad of a key event (arrows by physical key, logical arrows as a
/// fallback as in the other modes); Cross is X or Enter.
fn key_pad(event: &KeyboardInput) -> Pad {
    let mut pad = Pad::NONE;
    match event.key_code {
        KeyCode::ArrowUp => pad.up = true,
        KeyCode::ArrowDown => pad.down = true,
        KeyCode::ArrowLeft => pad.left = true,
        KeyCode::ArrowRight => pad.right = true,
        KeyCode::KeyX | KeyCode::Enter => pad.cross = true,
        _ => match event.logical_key {
            Key::ArrowUp => pad.up = true,
            Key::ArrowDown => pad.down = true,
            Key::ArrowLeft => pad.left = true,
            Key::ArrowRight => pad.right = true,
            _ => {}
        },
    }
    pad
}

fn map_keys(
    mut events: MessageReader<KeyboardInput>,
    mut focus_lost: MessageReader<KeyboardFocusLost>,
    mut runtime: ResMut<MapRuntime>,
) {
    for event in events.read() {
        runtime.keys.event(event);
    }
    if focus_lost.read().count() > 0 {
        runtime.keys.lose_focus();
    }
}

fn report_events(sim: &MapSim, events: &[MapEvent], trace: bool) {
    for event in events {
        match event {
            MapEvent::DuelRequest(request) => eprintln!(
                "Campaign map frame {}: BOUNDARY — duel against {} would start here (mode push parameters {:?}); {BOUNDARY}",
                sim.frame,
                sim.data.node(request.node).name,
                request.params
            ),
            MapEvent::DeckEditor { side } => eprintln!(
                "Campaign map frame {}: SYSTEM icon 0 pushes the deck editor (mode 6, side {side}; fade 0x3C frames not reproduced)",
                sim.frame
            ),
            MapEvent::SaveRequested => eprintln!("Campaign map: SYSTEM → SAVE (interim save file)"),
            MapEvent::TutorialLesson { row } => eprintln!(
                "Campaign map frame {}: SYSTEM → HELP row {row}: the lesson's duel starts (the card block is kept; back to the map after it)",
                sim.frame
            ),
            MapEvent::ReturnToTitle => eprintln!("Campaign map frame {}: Return to Title Screen? YES (0x1A9290)", sim.frame),
            MapEvent::OptionsSaved { bits } => eprintln!(
                "Campaign map frame {}: OPTION written (Monster Battle {}, Sound {})",
                sim.frame,
                if bits & roses_client::game_options::BATTLE_DISPLAY != 0 { "Display" } else { "Abbreviate" },
                if roses_client::game_options::stereo(*bits) { "Stereo" } else { "Mono" }
            ),
            MapEvent::SystemIconUntraced(icon) => eprintln!(
                "Campaign map frame {}: BOUNDARY — SYSTEM icon {icon} (save / sub-dialogs) is not traced",
                sim.frame
            ),
            MapEvent::StoryEvent(event) => eprintln!(
                "Campaign map frame {}: story event {event} (its pages play over the running map (11122, 11140..11148) or in the story scene (Stonehenge); the map continues afterwards)",
                sim.frame
            ),
            MapEvent::EventChanged(event) if map::MapModel::is_window_event(*event) => eprintln!(
                "Campaign map frame {}: story event {event}: letterbox and story window over the map (the map HUD is off screen)",
                sim.frame
            ),
            MapEvent::Arrived(node) => eprintln!(
                "Campaign map frame {}: arrived at {} (C2 stays {})",
                sim.frame,
                sim.data.node(*node).name,
                sim.save.event
            ),
            MapEvent::NodeBeaten(node) | MapEvent::NodeLost(node) if !trace => eprintln!(
                "Campaign map frame {}: {} state {} ({event:?})",
                sim.frame,
                sim.data.node(*node).name,
                sim.save.state(*node)
            ),
            other if trace => eprintln!("Campaign map frame {}: {other:?}", sim.frame),
            _ => {}
        }
    }
}

/// One original update at 25 Hz: held keys become the pad (Cross once per
/// press), the model steps, the title follows.
fn map_tick(
    mut runtime: ResMut<MapRuntime>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    capture: Option<Res<CaptureState>>,
    flow: Option<ResMut<super::GameFlow>>,
    sound: Option<ResMut<super::duel_board_render::DuelSound>>,
    recording: Option<Res<super::record::RecordTarget>>,
) {
    let runtime = &mut *runtime;
    runtime.stepped = false;
    let mut pad = runtime.keys.sample();
    // `--story-auto` also pages the story windows the map plays (11120,
    // 11124, 11149) and Mako's pages (12040, 12041): Cross as soon as the
    // page waits.
    let story_auto = flow.as_ref().is_some_and(|f| f.story_auto);
    // A recording has no window either: `--map-replay` is read here too.
    let mut flow = flow.filter(|_| capture.is_some() || recording.is_some());
    if let Some(flow) = flow.as_deref_mut() {
        let map_target = flow.capture_scene == Some(super::GameScene::Map);
        if flow.stopped.is_some()
            || (map_target && (flow.capture_map_gate.is_none() || flow.capture_map_reached))
        {
            // Capture the reached map before further input can leave it.
            return;
        }
        pad = capture_replay_pad(&mut flow.map_replay, &mut flow.map_replay_held);
    }
    if story_auto
        && (runtime.sim.story_window.as_ref().is_some_and(|w| w.phase == 4)
            || runtime.sim.story_dialogue.as_ref().is_some_and(|d| d.phase == 4))
    {
        pad.cross = true;
    }
    let mut card_pad = runtime.keys.take_card();
    if flow.is_some() {
        // Capture replays: the map pad's Cross and Left/Right.
        card_pad = roses_client::memory_card::CardPad {
            left: pad.left,
            right: pad.right,
            cross: pad.cross,
            ..Default::default()
        };
    }
    // SYSTEM → SAVE: while the card dialog runs the map takes no input (the
    // cursor stays on the icon [emu the project notes save-move]); the dialog
    // updates within the map update (0x237554).
    if runtime.card.is_some() {
        pad = Pad::NONE;
    }
    let events = runtime.sim.step(pad);
    runtime.stepped = true;
    runtime.story_cross = pad.cross;
    for event in &events {
        if let MapEvent::OptionsSaved { bits } = event {
            runtime.options_saved = Some(*bits);
        }
    }
    let mut card_sfx = Vec::new();
    if let Some((dialog, card)) = runtime.card.as_mut() {
        use roses_client::memory_card::CardOutcome;
        let outcome = dialog.update(card_pad, card);
        card_sfx = dialog.sfx.clone();
        let shown = dialog.overlay().map(|b| (b, dialog.counter));
        let finished = outcome != CardOutcome::Running;
        if finished {
            match &dialog.error {
                Some(e) => eprintln!("Campaign map: memory card: {e}"),
                None => eprintln!("Campaign map: memory-card dialog closed ({})", card.data_path().display()),
            }
        }
        if let Some(h) = runtime.hud.as_mut() {
            h.card_box = shown;
        }
        if finished {
            runtime.card = None;
        }
    }
    // `--capture-map-event E --capture-map-updates N`: the map capture waits
    // until the map has spent N updates at event E (any map entry).
    if let Some(flow) = flow.as_deref_mut() {
        if let Some((event, updates)) = flow.capture_map_gate {
            if runtime.sim.save.event == event {
                flow.capture_map_updates += 1;
                if flow.capture_map_updates >= updates && !flow.capture_map_reached {
                    flow.capture_map_reached = true;
                    let sim = &runtime.sim;
                    eprintln!(
                        "Game: map capture gate reached at event {event} after {updates} updates (handler {:?}, ship state {} sample {} at ({:.3}, {:.3}) heading {}, letterbox {} {:?}, caption {:?}, nodes {:?})",
                        sim.handler,
                        sim.ship.state,
                        sim.ship.index,
                        sim.ship.position[0],
                        sim.ship.position[2],
                        sim.ship.heading as i16,
                        sim.letterbox.active,
                        sim.letterbox.bars,
                        sim.caption.as_ref().map(|c| (c.message, c.phase, c.alpha)),
                        &sim.save.node_states[..12]
                    );
                }
            } else {
                flow.capture_map_updates = 0;
            }
        }
    }
    report_events(&runtime.sim, &events, runtime.trace);
    if let Some(mut sound) = sound {
        use roses_game::duel_audio::AudioCue;
        for event in &events {
            match event {
                // SYSTEM0 cues; 0x193 / 0x194 are effects (the ship, a
                // rising node), not sounds [F UI-SOUNDS.md].
                MapEvent::Sfx(id) if *id < 0x100 => sound.cue(AudioCue::Se { slot: 0, id: *id as u8, skip: false }),
                MapEvent::Music { track, keep } => sound.cue(AudioCue::Music { track: u32::from(*track), keep: *keep }),
                _ => {}
            }
        }
        // The memory-card dialog's SE 3/4/5 (0x216EA0(0, id)).
        for id in &card_sfx {
            sound.cue(AudioCue::Se { slot: 0, id: *id as u8, skip: false });
        }
    }
    let caption = title(&runtime.sim);
    if caption != runtime.title {
        for mut window in &mut windows {
            window.title = caption.clone();
        }
        runtime.title = caption;
    }
    let runtime = &mut *runtime;
    if let Some(h) = runtime.hud.as_mut() {
        if h.update(&runtime.sim) {
            runtime.layer_dirty = true;
        }
    }
    let items = without_hud_items(scene::layer_items(&runtime.sim), runtime.hud.is_some());
    if items != runtime.layer_items {
        runtime.layer_items = items;
        runtime.layer_dirty = true;
    }
}

/// Capture mode has no native window, but uses exactly the map's normal
/// pad contract: directions held, Cross once on the rising edge. Exhausting
/// the replay releases all held buttons instead of repeating its last input.
fn capture_replay_pad(replay: &mut std::collections::VecDeque<Pad>, held: &mut Pad) -> Pad {
    let next = replay.pop_front().unwrap_or(Pad::NONE);
    let edge = |now: bool, was: bool| now && !was;
    let m = next.menu;
    let h = held.menu;
    let menu = map::MenuPad {
        select: edge(m.select, h.select),
        circle: edge(m.circle, h.circle),
        square: edge(m.square, h.square),
        l1: edge(m.l1, h.l1),
        r1: edge(m.r1, h.r1),
        cross: edge(next.cross, held.cross),
        left: edge(next.left, held.left),
        right: edge(next.right, held.right),
        up: edge(next.up, held.up),
        down: edge(next.down, held.down),
    };
    let pad = Pad {
        cross: next.cross && !held.cross,
        cross_held: next.cross,
        menu,
        ..next
    };
    *held = next;
    pad
}

fn to_mat4(m: &camera::Mat4) -> Mat4 {
    Mat4::from_cols_array_2d(&m.map(|column| column.map(|v| v as f32)))
}
fn order(index: &mut u32) -> Transform {
    *index += 1;
    Transform::from_xyz(0.0, 0.0, -100_000.0 + *index as f32)
}

/// Push the camera and the instance transforms/visibility to the materials.
fn sync_scene(
    mut runtime: ResMut<MapRuntime>,
    input: Res<MapInput>,
    mut draws: Query<(
        &mut MapDrawEntity,
        &MeshMaterial3d<MapMaterial>,
        &mut Visibility,
    )>,
    mut materials: ResMut<Assets<MapMaterial>>,
) {
    let runtime = &mut *runtime;
    let Some(view) = scene::view(&runtime.sim) else {
        return;
    };
    let camera_moved = runtime.applied_target != Some(runtime.sim.camera_target);
    runtime.applied_target = Some(runtime.sim.camera_target);
    let instances: BTreeMap<String, Instance> = scene::instances(&runtime.sim, &input.loaded)
        .into_iter()
        .map(|i| (i.label.clone(), i))
        .collect();
    let figure = duel_board::translation(scene::figure_marker(&runtime.sim));
    for (mut entity, handle, mut visibility) in &mut draws {
        let (world, alpha, visible) = if entity.label == FIGURE_LABEL {
            (figure, 1.0, scene::figure_visible(&runtime.sim))
        } else {
            match instances.get(&entity.label) {
                Some(instance) => (instance.world, instance.alpha, true),
                None => (entity.world, entity.alpha, false),
            }
        };
        let want = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        if !visible {
            continue;
        }
        let moved = world != entity.world;
        if (moved || camera_moved) && alpha == entity.alpha {
            if let Some(mut material) = materials.get_mut(&handle.0) {
                material.ndc_from_source = to_mat4(&scene::ndc(&view, &world));
            }
        } else if moved || camera_moved || alpha != entity.alpha {
            if let Some(mut material) = materials.get_mut(&handle.0) {
                material.ndc_from_source = to_mat4(&scene::ndc(&view, &world));
                material.extra = Vec4::new(alpha, 0.0, 0.0, 0.0);
            }
        }
        entity.world = world;
        entity.alpha = alpha;
    }
}

/// The tooltip window's top-left (0x2361B8..0x236268) for a text image
/// `width` wide at `cursor`.
fn tooltip_origin(width: u32, cursor: (i32, i32)) -> (i64, i64) {
    let window = i64::from(width) + 16;
    let half = window >> 1;
    let (cx, cy) = (i64::from(cursor.0), i64::from(cursor.1));
    let mut x = cx - half;
    if x < 121 {
        x = 120;
    }
    if cx + half >= 370 {
        x = 370 - window;
    }
    let y = (cy + 32).clamp(128, 354);
    (x, y)
}

/// Premultiplied RGBA composition of the placeholder layer.
fn compose_layer(
    items: &[LayerItem],
    decoded: &BTreeMap<u16, (Vec<u8>, u32, u32)>,
    hud: Option<&[u8]>,
) -> Vec<u8> {
    // The HUD layer (premultiplied) first; the placeholder items over it.
    let mut layer = match hud {
        Some(rgba) if rgba.len() == (CANVAS * CANVAS * 4) as usize => rgba.to_vec(),
        _ => vec![0u8; (CANVAS * CANVAS * 4) as usize],
    };
    let mut put = |x: i64, y: i64, rgba: [u8; 4]| {
        if x < 0 || y < 0 || x >= i64::from(CANVAS) || y >= i64::from(CANVAS) || rgba[3] == 0 {
            return;
        }
        let at = ((y as u32 * CANVAS + x as u32) * 4) as usize;
        let a = u32::from(rgba[3]);
        for c in 0..3 {
            let src = u32::from(rgba[c]) * a / 255;
            let dst = u32::from(layer[at + c]) * (255 - a) / 255;
            layer[at + c] = (src + dst).min(255) as u8;
        }
        layer[at + 3] = (a + u32::from(layer[at + 3]) * (255 - a) / 255).min(255) as u8;
    };
    for item in items {
        match item {
            LayerItem::Text {
                message,
                x,
                y,
                lines,
            } => {
                let Some((pixels, width, height)) = decoded.get(message) else {
                    continue;
                };
                let rows = lines.map_or(*height, |l| (l * 22).min(*height));
                for row in 0..rows {
                    for column in 0..*width {
                        let at = ((row * width + column) * 4) as usize;
                        put(
                            i64::from(*x + column),
                            i64::from(*y + row),
                            [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]],
                        );
                    }
                }
            }
            LayerItem::Tooltip { message, cursor } => {
                let Some((pixels, width, height)) = decoded.get(message) else {
                    continue;
                };
                let (x, y) = tooltip_origin(*width, *cursor);
                for row in 0..*height {
                    for column in 0..*width {
                        let at = ((row * width + column) * 4) as usize;
                        put(
                            x + 8 + i64::from(column),
                            y + 4 + i64::from(row),
                            [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]],
                        );
                    }
                }
            }
            LayerItem::CenteredText { message, y } => {
                let Some((pixels, width, height)) = decoded.get(message) else {
                    continue;
                };
                let x = i64::from(CANVAS / 2) - i64::from(*width / 2);
                for row in 0..*height {
                    for column in 0..*width {
                        let at = ((row * width + column) * 4) as usize;
                        put(
                            x + i64::from(column),
                            i64::from(*y + row),
                            [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]],
                        );
                    }
                }
            }
            LayerItem::Caption {
                message,
                x,
                y,
                alpha,
            } => {
                let Some((pixels, width, height)) = decoded.get(message) else {
                    continue;
                };
                for row in 0..*height {
                    for column in 0..*width {
                        let at = ((row * width + column) * 4) as usize;
                        let a = u32::from(pixels[at + 3]) * u32::from(*alpha) / 255;
                        put(
                            i64::from(*x + column),
                            i64::from(*y + row),
                            [pixels[at], pixels[at + 1], pixels[at + 2], a as u8],
                        );
                    }
                }
            }
            LayerItem::Letterbox { height } => {
                let h = i64::from((*height).min(CANVAS));
                let last = i64::from(CANVAS);
                for y in (0..h).chain(last - h..last) {
                    for x in 0..last {
                        put(x, y, [0, 0, 0, 255]);
                    }
                }
            }
            LayerItem::Cursor { x, y } => {
                let half = i64::from(CURSOR_HALF);
                for d in -half..=half {
                    for (dx, dy) in [(d, 0), (0, d)] {
                        put(i64::from(*x) + dx, i64::from(*y) + dy, [255, 255, 255, 255]);
                    }
                }
                for d in -half - 1..=half + 1 {
                    for (dx, dy) in [(d, 1), (d, -1), (1, d), (-1, d)] {
                        let (px, py) = (i64::from(*x) + dx, i64::from(*y) + dy);
                        if dx.abs() > half || dy.abs() > half || (dx != 0 && dy != 0) {
                            put(px, py, [0, 0, 0, 160]);
                        }
                    }
                }
            }
        }
    }
    layer
}

fn sync_layer(
    mut runtime: ResMut<MapRuntime>,
    mut layer: ResMut<LayerImage>,
    mut images: ResMut<Assets<Image>>,
    mut presents: ResMut<Assets<PresentMaterial>>,
) {
    if !runtime.layer_dirty {
        return;
    }
    let Some(image) = layer.image.clone() else {
        return;
    };
    let pixels = compose_layer(
        &runtime.layer_items,
        &layer.decoded,
        runtime.hud.as_ref().map(|h| h.rgba.as_slice()),
    );
    if let Some(mut image) = images.get_mut(&image) {
        image.data = Some(pixels);
    }
    if let Some(present) = layer.present.clone() {
        presents.get_mut(&present);
    }
    runtime.layer_dirty = false;
    let _ = &mut layer;
}

pub(crate) fn load_png(bytes: &[u8], clamp_u: bool, clamp_v: bool, linear: bool) -> Result<Image, String> {
    let address = |clamp| {
        if clamp {
            ImageAddressMode::ClampToEdge
        } else {
            ImageAddressMode::Repeat
        }
    };
    let filter = if linear {
        ImageFilterMode::Linear
    } else {
        ImageFilterMode::Nearest
    };
    Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: address(clamp_u),
            address_mode_v: address(clamp_v),
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: ImageFilterMode::Nearest,
            lod_min_clamp: 0.,
            lod_max_clamp: 0.,
            ..default()
        }),
        RenderAssetUsages::default(),
    )
    .map_err(|e| format!("Texture decode failed: {e}"))
}

fn canvas_sampler(filter: ImageFilterMode) -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        mag_filter: filter,
        min_filter: filter,
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..default()
    })
}

/// Plain untextured pyramid (2 units wide, 10 high) marking the leader figure.
fn figure_mesh() -> Mesh {
    let positions: Vec<[f32; 3]> = vec![
        [-2.0, 0.0, -2.0],
        [2.0, 0.0, -2.0],
        [2.0, 0.0, 2.0],
        [-2.0, 0.0, 2.0],
        [0.0, 10.0, 0.0],
    ];
    let colors = vec![[255.0, 255.0, 255.0, 128.0]; 5];
    let uv = vec![[0.0, 0.0]; 5];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(vec![
        0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4, 0, 2, 1, 0, 3, 2,
    ]))
}

pub(crate) fn draw_mesh(draw: &DrawMesh) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, draw.positions_raw.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, draw.uv.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, draw.colors.clone())
    .with_inserted_indices(Indices::U32(draw.indices.clone()))
}

#[allow(clippy::too_many_arguments)]
fn setup_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<MapMaterial>>,
    mut presents: ResMut<Assets<PresentMaterial>>,
    input: Res<MapInput>,
    mut runtime: ResMut<MapRuntime>,
    mut layer: ResMut<LayerImage>,
    capture: Option<ResMut<CaptureState>>,
    bridge: Option<Res<CaptureBridge>>,
    mut exit: MessageWriter<AppExit>,
) {
    let result = (|| -> Result<(), String> {
        let runtime = &mut *runtime;
        let view = scene::view(&runtime.sim).ok_or("Map camera view is degenerate")?;
        runtime.applied_target = Some(runtime.sim.camera_target);
        let mut guard = bridge
            .as_ref()
            .map(|b| b.0.lock().expect("Capture progress mutex poisoned"));
        let mut progress = guard.as_deref_mut();
        let blank = images.add(Image::new_fill(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        ));
        if let Some(p) = progress.as_deref_mut() {
            p.textures.push(blank.id());
        }
        let mut textures: BTreeMap<(String, bool, bool, bool), Handle<Image>> = BTreeMap::new();
        let mut mesh_cache: BTreeMap<(u16, usize), Handle<Mesh>> = BTreeMap::new();
        let mut draw_index = 0u32;
        // Every possible instance exists from the start; the sync hides the
        // absent ones (hidden nodes still get sunk entities, dots fade in).
        let visible: BTreeMap<String, Instance> = scene::instances(&runtime.sim, &input.loaded)
            .into_iter()
            .map(|i| (i.label.clone(), i))
            .collect();
        let mut all = visible.clone();
        for (index, path) in input
            .loaded
            .data
            .paths
            .iter()
            .enumerate()
            .take(map::PATHS_PER_SIDE)
        {
            if path.sea_route {
                continue;
            }
            for (k, dot) in path.dots.iter().enumerate() {
                let label = format!("path {index} dot {k}");
                all.entry(label.clone()).or_insert_with(|| Instance {
                    slot: scene::RED_DOT_SLOT,
                    label,
                    world: camera::mul(
                        &duel_board::translation([f64::from(dot[0]), 0.0, f64::from(dot[1])]),
                        &duel_board::scale(input.loaded.models[&scene::RED_DOT_SLOT].scale),
                    ),
                    alpha: 0.0,
                });
            }
        }
        let ordered: Vec<&Instance> = {
            let mut list: Vec<&Instance> = all.values().collect();
            // Draw order: tiles, ship, nodes, dots, then the sea pieces.
            let rank = |i: &Instance| match i.slot {
                34..=39 => 0,
                61 => 1,
                62 => 3,
                40 | 41 => 4,
                _ => 2,
            };
            list.sort_by_key(|i| (rank(i), i.label.clone()));
            list
        };
        for instance in ordered {
            let model = &input.loaded.models[&instance.slot];
            let is_visible = visible.contains_key(&instance.label);
            for (index, draw) in model.draws.iter().enumerate() {
                let texture = match &draw.texture {
                    Some(path) => {
                        let key = (path.clone(), draw.clamp_u, draw.clamp_v, draw.linear);
                        match textures.get(&key) {
                            Some(handle) => handle.clone(),
                            None => {
                                let image = load_png(
                                    &input.loaded.textures[path],
                                    draw.clamp_u,
                                    draw.clamp_v,
                                    draw.linear,
                                )?;
                                let handle = images.add(image);
                                if let Some(p) = progress.as_deref_mut() {
                                    p.textures.push(handle.id());
                                }
                                textures.insert(key, handle.clone());
                                handle
                            }
                        }
                    }
                    None => blank.clone(),
                };
                let mesh = mesh_cache
                    .entry((instance.slot, index))
                    .or_insert_with(|| {
                        let handle = meshes.add(draw_mesh(draw));
                        if let Some(p) = progress.as_deref_mut() {
                            p.meshes.push(handle.id());
                        }
                        handle
                    })
                    .clone();
                let material = materials.add(MapMaterial {
                    ndc_from_source: to_mat4(&scene::ndc(&view, &instance.world)),
                    params: Vec4::new(
                        f32::from(u8::from(draw.textured)),
                        draw.alpha_greater_than.map_or(-1.0, f32::from),
                        0.0,
                        f32::from(u8::from(draw.texture_alpha)),
                    ),
                    texture,
                    dither_rows: Mat4::ZERO,
                    extra: Vec4::new(instance.alpha, 0.0, 0.0, 0.0),
                    fog: Vec4::ZERO,
                    fog_colour: Vec4::ZERO,
                    world_y: Vec4::ZERO,
                    blend: match draw.blend {
                        Blend::Opaque => 0,
                        Blend::Standard => 1,
                        Blend::Additive => 2,
                    },
                    z_masked: false,
                });
                if let Some(p) = progress.as_deref_mut() {
                    p.materials.push(material.id().untyped());
                }
                commands.spawn((
                    Name::new(format!(
                        "{} slot {} draw {index}",
                        instance.label, instance.slot
                    )),
                    MapDrawEntity {
                        label: instance.label.clone(),
                        world: instance.world,
                        alpha: instance.alpha,
                    },
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    order(&mut draw_index),
                    if is_visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                    NoFrustumCulling,
                    RenderLayers::layer(0),
                ));
            }
        }
        // Leader figure placeholder.
        let figure = duel_board::translation(scene::figure_marker(&runtime.sim));
        let mesh = meshes.add(figure_mesh());
        let material = materials.add(MapMaterial {
            ndc_from_source: to_mat4(&scene::ndc(&view, &figure)),
            params: Vec4::new(0.0, -1.0, 0.0, 0.0),
            texture: blank.clone(),
            dither_rows: Mat4::ZERO,
            extra: Vec4::new(1.0, 0.0, 0.0, 0.0),
            fog: Vec4::ZERO,
            fog_colour: Vec4::ZERO,
            world_y: Vec4::ZERO,
            blend: 0,
            z_masked: false,
        });
        if let Some(p) = progress.as_deref_mut() {
            p.meshes.push(mesh.id());
            p.materials.push(material.id().untyped());
        }
        commands.spawn((
            Name::new(FIGURE_LABEL),
            MapDrawEntity {
                label: FIGURE_LABEL.into(),
                world: figure,
                alpha: 1.0,
            },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            order(&mut draw_index),
            NoFrustumCulling,
            RenderLayers::layer(0),
        ));
        // Canvas and present pass (see duel_board_render.rs).
        let mut canvas = Image::new_target_texture(CANVAS, CANVAS, TextureFormat::Rgba8Unorm, None);
        let filter = if capture.is_some() {
            ImageFilterMode::Nearest
        } else {
            ImageFilterMode::Linear
        };
        canvas.sampler = canvas_sampler(filter);
        let canvas = images.add(canvas);
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(canvas.clone().into()),
            Transform::IDENTITY,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(0),
        ));
        for (id, (png, _, _)) in &input.loaded.text_images {
            let image = load_png(png, true, true, false)?;
            let data = image.data.clone().ok_or("Text image has no data")?;
            layer
                .decoded
                .insert(*id, (data, image.width(), image.height()));
        }
        runtime.layer_items = scene::layer_items(&runtime.sim);
        let mut layer_image = Image::new(
            Extent3d {
                width: CANVAS,
                height: CANVAS,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            compose_layer(
                &runtime.layer_items,
                &layer.decoded,
                runtime.hud.as_ref().map(|h| h.rgba.as_slice()),
            ),
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        layer_image.sampler = canvas_sampler(filter);
        let layer_image = images.add(layer_image);
        let no_units = images.add(Image::new_fill(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        ));
        // The cover canvas: a story played over the map (`MapStoryHost`),
        // over the HUD; transparent otherwise.
        let story_cover = images.add(super::ui_image(roses_client::Image {
            width: STORY_CANVAS[0],
            height: STORY_CANVAS[1],
            rgba: vec![0; (STORY_CANVAS[0] * STORY_CANVAS[1] * 4) as usize],
        }));
        layer.story = Some(story_cover.clone());
        let present = presents.add(PresentMaterial {
            canvas: canvas.clone(),
            // The shared board-present shader uses y as scene brightness.
            // The map has no tutorial fade: present its canvas at full level.
            params: Vec4::new(0., 1., 0., 0.),
            layer: layer_image.clone(),
            versus: no_units.clone(),
            history: no_units.clone(),
            top: no_units.clone(),
            cover: story_cover.clone(),
            blur: Vec4::ZERO,
            units: no_units,
            unit_tint: Vec4::ZERO,
            dither_rows: Mat4::ZERO,
        });
        layer.image = Some(layer_image.clone());
        layer.present = Some(present.clone());
        runtime.layer_dirty = false;
        let quad = meshes.add(Rectangle::new(2., 2.));
        if let Some(p) = progress.as_deref_mut() {
            p.textures.extend([canvas.id(), layer_image.id()]);
            p.meshes.push(quad.id());
            p.materials.push(present.id().untyped());
        }
        drop(guard);
        commands.spawn((
            Mesh3d(quad),
            MeshMaterial3d(present),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(1),
        ));
        let present_camera = commands
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 1,
                    ..default()
                },
                Transform::IDENTITY,
                Msaa::Off,
                Tonemapping::None,
                DebandDither::Disabled,
                RenderLayers::layer(1),
            ))
            .id();
        if let Some(mut capture) = capture {
            let mut target =
                Image::new_target_texture(CANVAS, CANVAS, TextureFormat::Rgba8UnormSrgb, None);
            target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
            let target = images.add(target);
            capture.target = Some(target.clone());
            commands
                .entity(present_camera)
                .insert(RenderTarget::Image(target.into()));
        } else {
            commands.entity(present_camera).insert(PresentationCamera);
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("Campaign map setup failed: {error}");
        exit.write(AppExit::error());
    }
}

use roses_game::duel_board;

/// Enter the campaign map inside the connected game (`--game`): at event 0
/// after the story, or back from a duel with its result.
/// How the game enters the map scene.
pub enum MapEntry {
    /// The story's end: event 0 after landing (state 622).
    EventZero,
    /// Back from a duel with its result (0x227F00 then 0x21C1A4).
    AfterDuel(bool),
    /// Back from a duel with the save the map left when it requested it.
    AfterDuelFrom(map::CampaignSave, bool),
    /// Back from the deck editor with the save the map left: the editor
    /// overwrote the map's work area, so the map re-initialises (0x21C1A4
    /// with the save's event; camera on the location) [H: the original's
    /// re-entry from mode 6 is not traced].
    Resume(map::CampaignSave),
    /// Back from a story stage of the story scene (`MapModel::after_story`:
    /// 11162 and 12132 open their closed letterbox over the map first).
    AfterStory(map::CampaignSave, i16),
}

pub fn enter_map_scene(
    world: &mut World,
    manifest: &std::path::Path,
    entry: MapEntry,
    system_icons: Vec<[i16; 4]>,
    deck_costs: Option<(u16, BTreeMap<u8, u16>)>,
    hud_source: Option<MapHudSource>,
) -> Result<(), String> {
    let loaded = scene::load_map(manifest)?;
    let figure_source = hud_source.as_ref().and_then(MapFigure::source);
    let mut sim = match entry {
        MapEntry::AfterDuel(won) => MapSim::after_duel(loaded.data.clone(), won)?,
        MapEntry::AfterDuelFrom(save, won) => MapSim::after_duel_from(loaded.data.clone(), save, won)?,
        MapEntry::EventZero => MapSim::at_event_zero(loaded.data.clone())?,
        MapEntry::Resume(save) => MapSim::resume(loaded.data.clone(), save)?,
        MapEntry::AfterStory(save, story) => MapSim::after_story(loaded.data.clone(), save, story)?,
    };
    if system_icons.is_empty() {
        eprintln!(
            "Campaign map: the SYSTEM icon rectangles were not given; the icons are a boundary"
        );
    }
    sim.system_icons = system_icons;
    match deck_costs {
        Some((player, opponents)) => {
            sim.player_dc = Some(player);
            sim.opponent_costs = opponents;
        }
        None => eprintln!("Campaign map: no deck costs; the DC check is off"),
    }
    let caption = title(&sim);
    let loaded_texanm = loaded.texanm.clone();
    if loaded_texanm.is_none() {
        eprintln!("Campaign map: this import has no TEXANM.MRG; the hover panel's face is not drawn");
    }
    let hud = match hud_source
        .map(|mut source| {
            source.texanm = loaded_texanm.clone();
            source
        })
        .map(MapHud::new)
    {
        Some(Ok(h)) => Some(h),
        Some(Err(e)) => {
            eprintln!("Campaign map: the HUD is unavailable ({e}); placeholders are drawn");
            None
        }
        None => {
            eprintln!("Campaign map: no duel import for the HUD; placeholders are drawn");
            None
        }
    };
    if let Some(h) = &hud {
        h.complete_messages(&mut sim);
    }
    world.insert_resource(MapInput { loaded });
    world.insert_resource(MapRuntime {
        sim,
        keys: MapKeys::default(),
        title: caption,
        trace: std::env::var("ROSES_TRACE_MAP").is_ok_and(|v| v == "1"),
        applied_target: None,
        layer_items: Vec::new(),
        layer_dirty: true,
        hud,
        card: None,
        options_saved: None,
        stepped: false,
        story_cross: false,
    });
    world.insert_resource(LayerImage {
        image: None,
        present: None,
        story: None,
        decoded: BTreeMap::new(),
    });
    if let Some(figure) = figure_source {
        world.insert_resource(figure);
    }
    world
        .run_system_cached(setup_map)
        .map_err(|e| format!("campaign map setup: {e}"))
}

/// Remove the map's resources (its entities are despawned by the game).
pub fn leave_map_scene(world: &mut World) {
    if let Some(host) = world.remove_resource::<MapStoryHost>() {
        world.resource_mut::<super::GameFlow>().parked_preview = Some(host.preview);
    }
    world.remove_resource::<MapFigure>();
    world.remove_resource::<MapInput>();
    world.remove_resource::<MapRuntime>();
    world.remove_resource::<LayerImage>();
}

/// SYSTEM icon 3's menu chose a lesson row (0x23B710 pushes the duel mode).
pub fn map_tutorial_request(world: &World) -> Option<u8> {
    world
        .get_resource::<MapRuntime>()
        .and_then(|r| r.sim.tutorial_request)
}

/// × on SYSTEM icon 0 pushed the deck editor (mode 6).
pub fn map_editor_request(world: &World) -> bool {
    world
        .get_resource::<MapRuntime>()
        .is_some_and(|r| r.sim.editor_request)
}

/// YES on "Return to Title Screen?" (0x1A9290 pops to the title).
pub fn map_title_request(world: &World) -> bool {
    world
        .get_resource::<MapRuntime>()
        .is_some_and(|r| r.sim.title_request)
}

/// The OPTION panel's written bits, taken once.
pub fn map_take_options(world: &mut World) -> Option<u8> {
    world
        .get_resource_mut::<MapRuntime>()
        .and_then(|mut r| r.options_saved.take())
}

/// The player's block options the panel starts from (0x214490).
pub fn map_set_option_bits(world: &mut World, bits: u8) {
    if let Some(mut r) = world.get_resource_mut::<MapRuntime>() {
        r.sim.option_bits = bits;
    }
}

/// The campaign save bytes as the map holds them now.
pub fn map_save(world: &World) -> Option<map::CampaignSave> {
    world
        .get_resource::<MapRuntime>()
        .map(|r| r.sim.save.clone())
}

/// SYSTEM → SAVE was chosen (taken once): the game writes its interim save.
pub fn map_save_request(world: &mut World) -> bool {
    world
        .get_resource_mut::<MapRuntime>()
        .is_some_and(|mut r| std::mem::take(&mut r.sim.save_request))
}

/// Start SYSTEM → SAVE's memory-card dialog (op 1) writing `file`.
pub fn map_start_save(
    world: &mut World,
    file: Vec<u8>,
    card: roses_game::memory_card_store::VirtualCard,
) {
    if let Some(mut r) = world.get_resource_mut::<MapRuntime>() {
        r.card = Some((roses_client::memory_card::CardDialog::save(file), card));
    }
}

/// The story event the map fired (C2 = 11120/11140/11160/11180), taken once:
/// the game plays its pages and resumes the map at the continuation.
pub fn map_story_request(world: &mut World) -> Option<i16> {
    world
        .get_resource_mut::<MapRuntime>()
        .and_then(|mut r| r.sim.take_story_request())
}

/// A story chain the original plays over the running map (11122 and
/// 11140..11148, `opening::plays_over_map`): the game's story model (its
/// parked `OpeningPreview`, motion and faces included) moves one update per
/// map update and its overlay is the present pass's cover canvas, over the
/// map and its HUD (sliding out under it). The map keeps drawing; black
/// fills, paintings and the closed letterbox of the overlay cover it where
/// the original's do [emu the project notes]. When the chain
/// reaches the map's window event (11124, 11149) the map plays it in place
/// (`MapModel::end_story`).
#[derive(Resource)]
struct MapStoryHost {
    preview: roses_game::opening::OpeningPreview,
    /// The stage whose music requests were made.
    music_stage: Option<String>,
    /// The overlay was drawn once (later only when the frame changed).
    drawn: bool,
}

/// One update of the hosted story (after the map's own update; the first
/// on the map update C2 became the event, as the story scene counts it).
fn map_story_tick(world: &mut World) {
    use roses_game::duel_audio::AudioCue;
    use roses_game::opening;
    let Some(runtime) = world.get_resource::<MapRuntime>() else { return };
    if !runtime.stepped {
        return;
    }
    let cross = runtime.story_cross;
    let pending = runtime.sim.story_request;
    if world.get_resource::<MapStoryHost>().is_none() {
        let Some(event) = pending else { return };
        let stage = opening::map_story_stage(event);
        if !opening::plays_over_map(&stage) || world.get_resource::<super::GameFlow>().is_none() {
            return;
        }
        let mut flow = world.resource_mut::<super::GameFlow>();
        // Without the game's story model (or the stage) the game's own
        // hand-off reports the boundary.
        let Some(mut preview) = flow.parked_preview.take() else { return };
        if !preview.model.stage_ids().contains(&stage.as_str()) {
            flow.parked_preview = Some(preview);
            return;
        }
        if let Err(e) = preview.resume_at(&stage) {
            eprintln!("Campaign map: story {stage} cannot start: {e}");
            flow.parked_preview = Some(preview);
            return;
        }
        preview.enable_motion(super::story_motion_enabled());
        world.resource_mut::<MapRuntime>().sim.take_story_request();
        eprintln!(
            "Game: the campaign map fired story stage {stage}; it plays over the running map (the map mode keeps drawing under its letterbox, figures and panels; its black fills and paintings cover it); Cross pages, the map continues at the chain's window event"
        );
        world.insert_resource(MapStoryHost { preview, music_stage: None, drawn: false });
    }
    let story_auto = world.get_resource::<super::GameFlow>().is_some_and(|f| f.story_auto);
    let auto_cross = story_auto && {
        let mut flow = world.resource_mut::<super::GameFlow>();
        flow.story_auto_ticks += 1;
        flow.story_auto_ticks % 8 == 0
    };
    let mut host = world.remove_resource::<MapStoryHost>().expect("host");
    // The stage's music requests (`opening::stage_music`), at its entry.
    let stage = host.preview.model.stage().id().to_owned();
    if host.music_stage.as_deref() != Some(stage.as_str()) {
        host.music_stage = Some(stage.clone());
        if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>() {
            if let Some(music) = host.preview.model.stage().music().cloned() {
                super::add_story_track(&mut sound, &host.preview, &music);
            }
            for cue in opening::stage_music(&stage) {
                sound.cue(cue);
            }
        }
    }
    let mut changed = host.preview.tick();
    let mut result: Result<(), String> = Ok(());
    // The motion's released Crosses first, then the pad's; nothing after
    // the chain handed back to the map.
    let mut map_event = None;
    let presses = host.preview.take_forwarded().into_iter().map(|b| (b, true));
    let pad = (cross || auto_cross).then_some((roses_client::Button::Cross, false));
    for (button, forwarded) in presses.chain(pad) {
        let outcome = if forwarded {
            host.preview.input_forwarded(button)
        } else {
            host.preview.input(button)
        };
        match outcome {
            Ok(repaint) => changed |= repaint,
            Err(e) => result = Err(e),
        }
        map_event = host.preview.take_map_event();
        if map_event.is_some() || result.is_err() {
            break;
        }
    }
    if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>() {
        for id in host.preview.take_sounds() {
            sound.cue(AudioCue::Se { slot: 0, id, skip: false });
        }
    }
    if let Err(message) = result {
        eprintln!("Game boundary: the story over the map stopped: {message}");
        world.resource_mut::<super::GameFlow>().stopped = Some(message);
        world.resource_mut::<super::GameFlow>().parked_preview = Some(host.preview);
        set_story_cover(world, None);
        return;
    }
    if let Some(event) = map_event {
        let event = i16::try_from(event).unwrap_or(0);
        let ended = world.resource_mut::<MapRuntime>().sim.end_story(event);
        world.resource_mut::<super::GameFlow>().parked_preview = Some(host.preview);
        set_story_cover(world, None);
        match ended {
            Ok(()) => eprintln!("Game: the story pages ended; the campaign map plays event {event} in place"),
            Err(message) => {
                eprintln!("Game boundary: {message}");
                world.resource_mut::<super::GameFlow>().stopped = Some(message);
            }
        }
        return;
    }
    // C2 follows the chain (11140 -> 11142 .. 11148).
    if let Some(event) = opening::map_story_event(host.preview.model.stage().id()) {
        world.resource_mut::<MapRuntime>().sim.story_progress(event);
    }
    if changed || !host.drawn {
        host.drawn = true;
        match host.preview.overlay() {
            Ok(image) => set_story_cover(world, Some(image)),
            Err(e) => eprintln!("Campaign map: story overlay: {e}"),
        }
    }
    world.insert_resource(host);
}

/// The present pass's cover canvas: the hosted story's overlay
/// (premultiplied, 640x448 over the whole frame), or transparent.
fn set_story_cover(world: &mut World, image: Option<roses_client::Image>) {
    let Some(layer) = world.get_resource::<LayerImage>() else { return };
    let (Some(handle), present) = (layer.story.clone(), layer.present.clone()) else {
        return;
    };
    let [w, h] = STORY_CANVAS;
    let image = image.unwrap_or(roses_client::Image {
        width: w,
        height: h,
        rgba: vec![0; (w * h * 4) as usize],
    });
    if let Some(mut target) = world.resource_mut::<Assets<Image>>().get_mut(&handle) {
        if (image.width, image.height) == (w, h) {
            target.data = Some(image.rgba);
        } else {
            *target = super::ui_image(image);
        }
    }
    // The material's bind group takes the new texture (as `sync_layer`).
    if let Some(present) = present {
        world.resource_mut::<Assets<PresentMaterial>>().get_mut(&present);
    }
}

/// The duel the map has asked for (0x232E40), once its walk and text end.
pub fn map_duel_request(world: &World) -> Option<map::DuelRequest> {
    world
        .get_resource::<MapRuntime>()
        .and_then(|r| r.sim.duel_request.clone())
}

/// The map materials, registered once for the whole app.
pub fn add_map_materials(app: &mut App) {
    app.add_plugins(MaterialPlugin::<MapMaterial>::default())
        .add_plugins(MaterialPlugin::<PresentMaterial>::default());
}

/// The live map systems, running only while a map scene exists.
pub fn add_map_systems(app: &mut App) {
    let active = resource_exists::<MapRuntime>;
    app.add_systems(
        RunFixedMainLoop,
        map_keys
            .run_if(active)
            .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(
        FixedUpdate,
        (map_tick, map_story_tick, sync_scene, sync_layer, sync_figure).chain().run_if(active),
    );
}

/// The leader figure (unit object 30, `campaign_map::figure`): the player's
/// current-deck leader from MONSTER.MRG, idle motion 0 looping, placed by
/// `MapModel::figure_object` every update and drawn with the duel's model
/// materials in the map canvas after the map's draws. [H] Lit with the
/// board-unit light (the map's light set is untraced); an alpha below 0x80
/// is drawn opaque.
#[derive(Resource)]
struct MapFigure {
    manifest: std::path::PathBuf,
    card: u16,
    height: i8,
    built: Option<FigureModel>,
    failed: bool,
}

struct FigureModel {
    scene: roses_game::duel_models::UnitScene,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<crate::duel_model_render::ModelMaterial>>,
    entities: Vec<Entity>,
}

/// Painter depth of the figure's groups: after every map draw.
const FIGURE_FIRST_DEPTH: f32 = -100_000.0 + 20_000.0;

impl MapFigure {
    fn source(source: &MapHudSource) -> Option<Self> {
        let manifest = source.models_manifest.clone()?;
        let heights = match map::figure_heights(&source.executable) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("Campaign map: the leader figure's height table is unavailable ({e})");
                return None;
            }
        };
        let card = source.deck.leader;
        Some(Self {
            manifest,
            card,
            height: map::figure_height(&heights, card).unwrap_or(0),
            built: None,
            failed: false,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_figure(
    mut commands: Commands,
    runtime: Res<MapRuntime>,
    figure: Option<ResMut<MapFigure>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::duel_model_render::ModelMaterial>>,
    mut markers: Query<(&MapDrawEntity, &mut Visibility)>,
) {
    use crate::duel_model_render::{ModelTextures, dither_mat4, mesh_of, model_material};
    use roses_game::duel_models::{self, UnitScene};
    let Some(mut figure) = figure else { return };
    let Some(view) = scene::view(&runtime.sim) else { return };
    let ndc = to_mat4(&scene::ndc(&view, &camera::identity()));
    if figure.built.is_none() && !figure.failed {
        let built = (|| -> Result<FigureModel, String> {
            let loaded = duel_models::load_model(&figure.manifest, figure.card)?;
            let scene = UnitScene::new(loaded, map::FIGURE_MOTION, (3, 3), 0, 1.0)?;
            let frame = scene.frame()?;
            let dither = dither_mat4(&scene.loaded.dither);
            let mut textures = ModelTextures::default();
            let (mut mesh_handles, mut material_handles, mut entities) = (Vec::new(), Vec::new(), Vec::new());
            for (order, (group, mesh)) in scene.groups.iter().zip(&frame).enumerate() {
                let mut created = Vec::new();
                let texture = textures.get(&mut images, &scene.loaded, group, &mut created)?;
                let mesh = meshes.add(mesh_of(mesh));
                let material = materials.add(model_material(&group.state, ndc, texture, dither, true));
                entities.push(
                    commands
                        .spawn((
                            Name::new(format!("map leader figure group {order}")),
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            Transform::from_xyz(0.0, 0.0, FIGURE_FIRST_DEPTH + order as f32)
                                .with_scale(Vec3::splat(duel_models::DRAW_ORDER_SCALE)),
                            NoFrustumCulling,
                            RenderLayers::layer(0),
                        ))
                        .id(),
                );
                mesh_handles.push(mesh);
                material_handles.push(material);
            }
            Ok(FigureModel { scene, meshes: mesh_handles, materials: material_handles, entities })
        })();
        match built {
            Ok(model) => {
                eprintln!("Campaign map: the leader figure is card {} (height {})", figure.card, figure.height);
                figure.built = Some(model);
            }
            Err(e) => {
                eprintln!("Campaign map: the leader figure (card {}) is not drawn: {e}", figure.card);
                figure.failed = true;
                return;
            }
        }
    }
    let height = figure.height;
    let Some(model) = figure.built.as_mut() else { return };
    // The placeholder marker gives way to the model.
    for (entity, mut visibility) in &mut markers {
        if entity.label == FIGURE_LABEL && *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
    let object = runtime.sim.figure_object(height);
    let shown = object.visible && object.alpha > 0;
    for entity in &model.entities {
        commands
            .entity(*entity)
            .insert(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
    scene::place_figure(&mut model.scene, &object);
    if let Ok(frame) = model.scene.frame() {
        if std::env::var("ROSES_TRACE_FIGURE").is_ok() && runtime.sim.frame % 25 == 0 {
            let v = frame.first().and_then(|m| m.positions.first().copied());
            eprintln!("figure f{} obj {:?} first vertex {:?}", runtime.sim.frame, object, v);
        }
        for (handle, mesh) in model.meshes.iter().zip(&frame) {
            if let Some(mut target) = meshes.get_mut(handle) {
                *target = mesh_of(mesh);
            }
        }
    }
    for handle in &model.materials {
        if let Some(mut material) = materials.get_mut(handle) {
            material.set_ndc(ndc);
        }
    }
    // The main loop advances every object's clock once per update.
    model.scene.tick();
}

pub fn run(args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let options = parse(args)?;
    let loaded = scene::load_map(&options.manifest)?;
    let mut sim = match options.result {
        Some(won) => MapSim::after_duel(loaded.data.clone(), won)?,
        None => MapSim::at_event_zero(loaded.data.clone())?,
    };
    let trace = std::env::var("ROSES_TRACE_MAP").is_ok_and(|v| v == "1");
    let ticks = options.ticks.unwrap_or(options.replay.len() as u32);
    for tick in 0..ticks as usize {
        let pad = options.replay.get(tick).copied().unwrap_or(Pad::NONE);
        let events = sim.step(pad);
        report_events(&sim, &events, trace);
    }
    let instances = scene::instances(&sim, &loaded);
    let draws: usize = instances
        .iter()
        .map(|i| loaded.models[&i.slot].draws.len())
        .sum();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "campaign_map_manifest_sha256": loaded.manifest_sha256,
            "model_archive_sha256": loaded.manifest.source.model_archive.sha256,
            "start": if let Some(won) = options.result { if won { "return-win" } else { "return-loss" } } else { "event-0" },
            "updates_run": ticks,
            "frame": sim.frame,
            "event_c2": sim.save.event,
            "node_states": sim.save.node_states,
            "cursor": [sim.cursor.0, sim.cursor.1],
            "hovered": sim.hovered,
            "camera_target": sim.camera_target,
            "camera_eye": map::camera_eye(sim.camera_target),
            "anchors": scene::anchor_report(&sim),
            "travel": sim.travel,
            "figure": {"state": sim.figure.state, "position": sim.figure.position},
            "text": sim.text.as_ref().map(|t| serde_json::json!({"message": t.message, "phase": t.phase, "openness": t.openness})),
            "duel_request": sim.duel_request,
            "rose_cards": sim.save.rose_cards(&sim.data),
            "notes": sim.notes,
            "instances": instances.len(),
            "draws": draws,
            "models": loaded.models.len(),
            "not_implemented": [
                "HUD, hover panel, portraits and window art (placeholder message images and crosshair)",
                "leader figure model (plain marker)", "ship routes (parked)", "sea piece motion",
                "square scroll mode and SYSTEM icons", "DC check", "16-bit frame buffer dither", "sound"
            ],
            "renderer_parity_validated": false,
        }))
        .map_err(|e| e.to_string())?
    );
    if options.validate_only {
        return Ok(());
    }
    let _watchdog = options.capture.as_ref().map(|_| CaptureWatchdog::start(0));
    let caption = title(&sim);
    let runtime = MapRuntime {
        sim,
        keys: MapKeys::default(),
        title: caption.clone(),
        trace,
        applied_target: None,
        layer_items: Vec::new(),
        layer_dirty: true,
        hud: None,
        card: None,
        options_saved: None,
        stepped: false,
        story_cross: false,
    };
    let mut app = App::new();
    let mut plugins = DefaultPlugins.set(AssetPlugin {
        file_path: roses_game::asset_folder(),
        ..default()
    });
    if options.capture.is_some() {
        plugins = plugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>();
    } else {
        plugins = plugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: caption,
                resolution: (960, 720).into(),
                ..default()
            }),
            ..default()
        });
    }
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(MapInput { loaded })
        .insert_resource(runtime)
        .insert_resource(LayerImage {
            image: None,
            present: None,
            story: None,
            decoded: BTreeMap::new(),
        })
        .add_plugins(plugins)
        .add_plugins(MaterialPlugin::<MapMaterial>::default())
        .add_plugins(MaterialPlugin::<PresentMaterial>::default())
        .add_plugins(MaterialPlugin::<CloudMaterial>::default())
        .add_systems(Startup, setup_map);
    let mut capture_result = None;
    if let Some(path) = options.capture {
        let bridge = CaptureBridge::default();
        capture_result = Some(bridge.clone());
        app.insert_resource(bridge.clone())
            .insert_resource(CaptureState {
                path,
                target: None,
                started: Instant::now(),
                requested: false,
                completed: false,
                settled_frames: 0,
            })
            .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            )))
            .add_systems(Update, drive_capture);
        app.get_sub_app_mut(RenderApp)
            .ok_or("Bevy render sub-app unavailable")?
            .insert_resource(bridge)
            .add_systems(Render, inspect_render_progress.after(RenderSystems::Render));
    } else {
        // The gamepads press pad 1's keys (pad_input).
        crate::pad_input::add_systems(&mut app);
        // Original updates run once per two PAL fields (25 Hz, `UPDATE_HZ`).
        app.insert_resource(Time::<Fixed>::from_hz(roses_game::UPDATE_HZ))
            .add_systems(
                RunFixedMainLoop,
                map_keys.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(FixedUpdate, (map_tick, sync_scene, sync_layer).chain())
            .add_systems(Update, (fit_view, exit_on_escape));
    }
    let outcome = app.run();
    if !outcome.is_success() {
        return Err(format!("Bevy renderer exited with {outcome:?}"));
    }
    if let Some(bridge) = capture_result {
        if !bridge.0.lock().map_err(|e| e.to_string())?.saved {
            return Err("Renderer exited before a validated capture was saved".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_replay_preserves_held_directions_and_cross_edges_without_a_window() {
        let held_cross = Pad {
            up: true,
            cross: true,
            cross_held: true,
            ..Pad::NONE
        };
        let mut replay = [held_cross, held_cross, Pad::NONE, held_cross].into();
        let mut held = Pad::NONE;
        // The first update's edges also reach the dialogs (Cross, Up).
        let first = capture_replay_pad(&mut replay, &mut held);
        assert!(first.menu.cross && first.menu.up && !first.menu.select);
        assert_eq!(Pad { menu: map::MenuPad::NONE, ..first }, held_cross);
        let next = capture_replay_pad(&mut replay, &mut held);
        assert!(!next.menu.cross && !next.menu.up);
        assert!(next.up && !next.cross);
        assert_eq!(capture_replay_pad(&mut replay, &mut held), Pad::NONE);
        assert_eq!(Pad { menu: map::MenuPad::NONE, ..capture_replay_pad(&mut replay, &mut held) }, held_cross);
        assert_eq!(capture_replay_pad(&mut replay, &mut held), Pad::NONE);
        assert_eq!(held, Pad::NONE);
    }

    fn event(physical: KeyCode, logical: Key) -> KeyboardInput {
        KeyboardInput {
            key_code: physical,
            logical_key: logical,
            state: ButtonState::Pressed,
            repeat: false,
            text: None,
            window: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn a_press_and_release_in_one_host_batch_reaches_exactly_one_map_tick() {
        for (key, logical) in [
            (KeyCode::ArrowUp, Key::ArrowUp),
            (KeyCode::ArrowDown, Key::ArrowDown),
            (KeyCode::ArrowLeft, Key::ArrowLeft),
            (KeyCode::ArrowRight, Key::ArrowRight),
        ] {
            let mut input = event(key, logical);
            let expected = key_pad(&input);
            let mut keys = MapKeys::default();
            keys.event(&input);
            input.state = ButtonState::Released;
            keys.event(&input);
            // The new press also reaches the dialogs (`Pad::menu`).
            let sampled = keys.sample();
            assert!(sampled.menu.up || sampled.menu.down || sampled.menu.left || sampled.menu.right);
            assert_eq!(Pad { menu: map::MenuPad::NONE, ..sampled }, expected);
            assert_eq!(keys.sample(), Pad::NONE);
        }
    }

    #[test]
    fn tooltip_follows_the_cursor_inside_the_originals_clamps() {
        // 0x2361B8: Chester's anchor (243, 135), a 56-px text: window 72.
        assert_eq!(tooltip_origin(56, (243, 135)), (207, 167));
        // Left and right clamps (x ≥ 120, x + w < 370) and y 128..=354.
        assert_eq!(tooltip_origin(56, (0, 0)), (120, 128));
        assert_eq!(tooltip_origin(56, (480, 480)), (298, 354));
    }

    #[test]
    fn dialog_rows_repeat_with_the_pad_counter_not_the_host() {
        let mut keys = MapKeys::default();
        let mut down = event(KeyCode::ArrowDown, Key::ArrowDown);
        keys.event(&down);
        let mut fires = Vec::new();
        for update in 0..30 {
            if update == 5 {
                // Host key repeats change nothing.
                down.repeat = true;
                keys.event(&down);
                keys.event(&down);
            }
            if keys.sample().menu.down {
                fires.push(update);
            }
        }
        // The press, then 0x203380 mode 2: 13 updates later and every update.
        assert_eq!(fires, [0].into_iter().chain(13..30).collect::<Vec<_>>());
        down.repeat = false;
        down.state = ButtonState::Released;
        keys.event(&down);
        // One more on the update the release is seen (older sample pair).
        let s = keys.sample();
        assert!(s.menu.down && !s.down);
        assert!(!keys.sample().menu.down);
    }

    #[test]
    fn held_direction_repeats_per_tick_and_release_stops_it() {
        let mut keys = MapKeys::default();
        let mut up = event(KeyCode::ArrowUp, Key::ArrowUp);
        keys.event(&up);
        assert!(keys.sample().up);
        assert!(keys.sample().up);
        up.repeat = true;
        keys.event(&up);
        assert!(keys.sample().up);
        up.state = ButtonState::Released;
        keys.event(&up);
        assert_eq!(keys.sample(), Pad::NONE);
    }

    #[test]
    fn cross_is_one_shot_and_focus_loss_clears_pending_and_held_inputs() {
        let mut keys = MapKeys::default();
        let cross = event(KeyCode::Enter, Key::Enter);
        keys.event(&cross);
        assert!(keys.sample().cross);
        // Held but not newly pressed (the over-DC message reads the hold).
        let held = Pad {
            cross_held: true,
            ..Pad::NONE
        };
        assert_eq!(keys.sample(), held);
        let mut repeat = cross.clone();
        repeat.repeat = true;
        keys.event(&repeat);
        assert_eq!(keys.sample(), held);
        let mut release = cross.clone();
        release.state = ButtonState::Released;
        keys.event(&release);
        assert_eq!(keys.sample(), Pad::NONE);
        keys.event(&event(KeyCode::ArrowUp, Key::ArrowUp));
        keys.event(&cross);
        keys.lose_focus();
        assert_eq!(keys.sample(), Pad::NONE);
        assert_eq!(keys.sample(), Pad::NONE);
        assert!(keys.held.is_empty());
    }

    #[test]
    fn keys_map_to_the_map_pad() {
        assert!(key_pad(&event(KeyCode::ArrowUp, Key::ArrowUp)).up);
        assert!(key_pad(&event(KeyCode::KeyX, Key::Character("x".into()))).cross);
        assert!(key_pad(&event(KeyCode::Enter, Key::Enter)).cross);
        assert!(key_pad(&event(KeyCode::Numpad6, Key::ArrowRight)).right);
        assert_eq!(
            key_pad(&event(KeyCode::KeyA, Key::Character("a".into()))),
            Pad::NONE
        );
    }

    #[test]
    fn options_validate_result_replay_and_ticks() {
        let args = |s: &str| {
            s.split(' ')
                .map(OsString::from)
                .collect::<Vec<_>>()
                .into_iter()
        };
        let o = parse(args("--campaign-map m.json")).unwrap();
        assert!(o.result.is_none() && o.replay.is_empty() && o.ticks.is_none());
        let o = parse(args(
            "--campaign-map m.json --map-result win --replay up*5,cross --ticks 60",
        ))
        .unwrap();
        assert_eq!(
            (o.result, o.replay.len(), o.ticks),
            (Some(true), 6, Some(60))
        );
        assert!(parse(args("--campaign-map m.json --map-result draw")).is_err());
        assert!(parse(args("--campaign-map m.json --replay up*5 --ticks 2")).is_err());
        assert!(
            parse(args(
                "--campaign-map m.json --validate-only --capture x.png"
            ))
            .is_err()
        );
        assert!(parse(args("--map-result win")).is_err());
    }

    #[test]
    fn layer_composition_is_premultiplied_and_clipped() {
        let mut decoded = BTreeMap::new();
        decoded.insert(7u16, (vec![255, 0, 0, 255, 0, 0, 255, 128], 2, 1));
        let items = vec![
            LayerItem::Text {
                message: 7,
                x: 510,
                y: 0,
                lines: None,
            },
            LayerItem::Cursor { x: 0, y: 0 },
        ];
        let layer = compose_layer(&items, &decoded, None);
        assert_eq!(
            &layer[(510 * 4) as usize..(510 * 4 + 4) as usize],
            &[255, 0, 0, 255]
        );
        assert_eq!(
            &layer[(511 * 4) as usize..(511 * 4 + 4) as usize],
            &[0, 0, 128, 128]
        );
        // Cursor centre is white; off-canvas pixels are skipped.
        assert_eq!(&layer[0..4], &[255, 255, 255, 255]);
        assert_eq!(layer.len(), (CANVAS * CANVAS * 4) as usize);
    }
}
