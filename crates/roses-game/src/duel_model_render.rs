//! `--duel-model <models manifest> --card <id>`: one original duel unit model
//! (MONSTER.MRG, from a private `roses import-duel-models` import) standing on
//! a board cell, drawn with the original per-vertex lighting in the board's
//! GS projection (x = 256 + 800 x/z, y = 256 + 1074.2857 y/z, 512x512
//! canvas) from the native board camera, animating its motion at 25 Hz.
//! Scene rules live in the GPU-free `duel_models` module; this file only
//! drives Bevy and does not depend on the board renderer, so the same
//! module can later place models on the board.
//!
//! Draw path: the CPU evaluates the pose and skins and lights every vertex
//! once per update; each GS draw state becomes one mesh (world positions,
//! GS vertex colours, S/T) in a Rgba8Unorm canvas, so blending works on GS
//! byte values. Opaque draws dither into the 16-bit frame buffer with the
//! executable's unit DIMX entry; blended draws are fixed-function and are
//! only truncated to 5 bits by the capture's present pass.
use super::{
    CaptureBridge, CaptureState, CaptureWatchdog, PresentationCamera, drive_capture,
    exit_on_escape, fit_view,
};
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
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey, PreparedMaterial},
    prelude::*,
    reflect::TypePath,
    render::{
        Render, RenderApp, RenderSystems,
        erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
            CachedPipelineState, CompareFunction, Extent3d, PipelineCache, PipelineDescriptor,
            PrimitiveTopology, RenderPipelineDescriptor, SpecializedMeshPipelineError,
            TextureDimension, TextureFormat, TextureUsages,
        },
        texture::GpuImage,
    },
    shader::{ShaderCacheError, ShaderRef},
    window::ExitCondition,
    winit::WinitPlugin,
};
use roses_game::duel_board::{self, BoardCamera};
use roses_game::duel_models::{self, Blend, FrameMesh, UnitScene};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};

pub const PIPELINE: &str = "roses-duel-model";
const PRESENT_PIPELINE: &str = "roses-duel-model-present";
const NOT_IMPLEMENTED: &str =
    "motion cross-fades, the cursor unit's offscreen composite, board, cards and HUD not drawn";

fn is_model_pipeline(label: &str) -> bool {
    matches!(label, PIPELINE | PRESENT_PIPELINE)
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(ModelKey)]
pub struct ModelMaterial {
    #[uniform(0)]
    ndc_from_world: Mat4,
    /// x: PRIM TME, y: ATST GREATER reference (-1 = off), z: 16-bit dithered
    /// write, w: TEX0 TCC.
    #[uniform(1)]
    params: Vec4,
    #[texture(2)]
    #[sampler(3)]
    texture: Handle<Image>,
    #[uniform(4)]
    dither_rows: Mat4,
    /// x: 0 opaque, 1 ALPHA 0x44, 2 ALPHA 0x48, 3 ALPHA 0x42; y/z: CLAMP
    /// WMS/WMT clamp; w: TEX1 bilinear (the shader filters texels itself).
    #[uniform(5)]
    mode: Vec4,
    /// Height fog: (Q0, Q1, 1 = on, 0); off by default (the board).
    #[uniform(6)]
    fog: Vec4,
    /// FOGCOL RGB bytes.
    #[uniform(7)]
    fog_colour: Vec4,
    blend: u8,
}
#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct ModelKey {
    blend: u8,
}
impl From<&ModelMaterial> for ModelKey {
    fn from(material: &ModelMaterial) -> Self {
        Self {
            blend: material.blend,
        }
    }
}
impl Material for ModelMaterial {
    fn vertex_shader() -> ShaderRef {
        "duel_model.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "duel_model.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        // Every group goes through the sorted phase, whose key
        // (`group_transform`) is the original draw order: opaque pass, then
        // the blended pass. Blending itself is set in `specialize`.
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
        descriptor.label = Some(PIPELINE.into());
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?];
        // Both windings are drawn (no culling in the dump).
        descriptor.primitive.cull_mode = None;
        let keep = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        let color = match key.bind_group_data.blend {
            1 => Some(BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            }),
            2 => Some(BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            }),
            3 => Some(BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::ReverseSubtract,
            }),
            _ => None,
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = color.map(|color| BlendState { color, alpha: keep });
            }
        }
        if let Some(depth) = &mut descriptor.depth_stencil {
            // ZTE 1, ZTST GREATER; Z writes stay on for the blended pass
            // too (its ZMSK is not in the model's packets; unresolved).
            depth.depth_compare = Some(CompareFunction::Greater);
            depth.depth_write_enabled = Some(true);
        }
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ModelPresentMaterial {
    #[texture(0)]
    #[sampler(1)]
    canvas: Handle<Image>,
    #[uniform(2)]
    params: Vec4,
}
impl Material for ModelPresentMaterial {
    fn vertex_shader() -> ShaderRef {
        "duel_model_present.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "duel_model_present.wgsl".into()
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

#[derive(Debug)]
struct Options {
    manifest: PathBuf,
    card: u16,
    motion: u8,
    time: Option<f32>,
    cell: (u8, u8),
    side: u8,
    cursor: Option<(u8, u8)>,
    brightness: f64,
    ticks: u32,
    gs16: bool,
    capture: Option<PathBuf>,
    validate_only: bool,
}

fn text(value: Option<OsString>, what: &str) -> Result<String, String> {
    value
        .ok_or(format!("{what} requires a value"))?
        .into_string()
        .map_err(|_| format!("{what} must be UTF-8"))
}
fn number<T: std::str::FromStr>(value: Option<OsString>, what: &str) -> Result<T, String> {
    let text = text(value, what)?;
    text.parse().map_err(|_| format!("Invalid {what}: {text}"))
}

fn parse(args: impl Iterator<Item = OsString>) -> Result<Options, String> {
    let mut options = Options {
        manifest: PathBuf::new(),
        card: 0,
        motion: 0,
        time: None,
        cell: (3, 6),
        side: 0,
        cursor: None,
        brightness: 1.0,
        ticks: 0,
        gs16: true,
        capture: None,
        validate_only: false,
    };
    let (mut manifest, mut card) = (None, None);
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--duel-model") if manifest.is_none() => {
                manifest = Some(PathBuf::from(
                    args.next().ok_or("--duel-model requires a manifest path")?,
                ));
            }
            Some("--card") if card.is_none() => card = Some(number::<u16>(args.next(), "--card")?),
            Some("--motion") => options.motion = number(args.next(), "--motion")?,
            Some("--time") => {
                let t: f32 = number(args.next(), "--time")?;
                if !(0.0..=10_000.0).contains(&t) {
                    return Err("--time must be 0..=10000 motion frames".into());
                }
                options.time = Some(t);
            }
            Some("--cell") => {
                options.cell = duel_models::parse_cell(&text(args.next(), "--cell")?)?
            }
            Some("--cursor") => {
                options.cursor = Some(duel_models::parse_cell(&text(args.next(), "--cursor")?)?)
            }
            Some("--side") => {
                options.side = number(args.next(), "--side")?;
                if options.side > 1 {
                    return Err("--side must be 0 or 1".into());
                }
            }
            Some("--brightness") => {
                options.brightness = number(args.next(), "--brightness")?;
                if !(0.0..=1.0).contains(&options.brightness) {
                    return Err("--brightness is the lighting factor g, 0..=1".into());
                }
            }
            Some("--ticks") => {
                options.ticks = number(args.next(), "--ticks")?;
                if options.ticks > 100_000 {
                    return Err("--ticks exceeds 100000".into());
                }
            }
            Some("--rgb888") => options.gs16 = false,
            Some("--validate-only") => options.validate_only = true,
            Some("--capture") if options.capture.is_none() => {
                options.capture = Some(PathBuf::from(
                    args.next().ok_or("--capture requires a new PNG path")?,
                ));
            }
            Some("--help") => {
                println!(
                    "roses-bevy-scene --duel-model <duel-models manifest.json> --card <id> [--motion n] [--time t | --ticks n] [--cell c,r] [--side 0|1] [--cursor c,r] [--brightness g] [--rgb888] [--validate-only | --capture <new.png>]\nDraws one unit model from `roses import-duel-models` on board cell --cell (default 3,6) for --side (0: rotation Y 0x8000), seen by the native board camera at --cursor (default: the cell), lit by the board unit light with brightness g (default 1). The motion (default 0 = idle) starts at SetMotion's frame 1 (or --time) and advances one frame per 25 Hz update; --ticks runs updates before the first frame. Escape closes. {NOT_IMPLEMENTED}."
                );
                std::process::exit(0);
            }
            Some("--faithful") => {}
            _ => {
                return Err(format!(
                    "Unknown duel-model argument: {}",
                    arg.to_string_lossy()
                ));
            }
        }
    }
    options.manifest = manifest.ok_or("--duel-model requires a manifest path")?;
    options.card = card.ok_or("--duel-model requires --card <id>")?;
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

pub(crate) fn to_mat4(m: &roses_game::camera::Mat4) -> Mat4 {
    Mat4::from_cols_array_2d(&m.map(|column| column.map(|v| v as f32)))
}
/// DIMX rows as matrix columns (the shader indexes [y & 3][x & 3]).
pub(crate) fn dither_mat4(m: &[[i8; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(&m.map(|row| row.map(f32::from)))
}
pub(crate) fn mesh_of(frame: &FrameMesh) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, frame.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, frame.uv.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, frame.colours.clone())
}
/// Entity transform of group `order`. The shader ignores it; Bevy's sorted
/// phase keys on `transform * mesh AABB centre`, which this pins to the
/// original draw order for any mesh within `DRAW_ORDER_EXTENT`.
fn group_transform(order: usize) -> Transform {
    Transform::from_xyz(0.0, 0.0, duel_models::draw_order_depth(order))
        .with_scale(Vec3::splat(duel_models::DRAW_ORDER_SCALE))
}
fn blend_code(blend: Option<Blend>) -> u8 {
    match blend {
        None => 0,
        Some(Blend::Normal) => 1,
        Some(Blend::Additive) => 2,
        Some(Blend::Subtractive) => 3,
    }
}

/// GPU textures of a model's draw groups, created once per (texture, clamp,
/// filter); untextured groups sample a 1x1 blank (created only when used,
/// so every tracked image is referenced by a material).
#[derive(Default)]
pub(crate) struct ModelTextures {
    cache: BTreeMap<(usize, bool, bool, bool), Handle<Image>>,
    blank: Option<Handle<Image>>,
}

impl ModelTextures {
    /// The texture of `group`; new images are listed in `created`.
    pub(crate) fn get(
        &mut self,
        images: &mut Assets<Image>,
        loaded: &duel_models::LoadedModel,
        group: &duel_models::DrawGroup,
        created: &mut Vec<AssetId<Image>>,
    ) -> Result<Handle<Image>, String> {
        let state = &group.state;
        let Some(index) = group.texture else {
            return Ok(self
                .blank
                .get_or_insert_with(|| {
                    let handle = images.add(Image::new_fill(
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
                    created.push(handle.id());
                    handle
                })
                .clone());
        };
        let key = (index, state.clamp_u, state.clamp_v, state.linear);
        if let Some(handle) = self.cache.get(&key) {
            return Ok(handle.clone());
        }
        let address = |clamp| {
            if clamp {
                ImageAddressMode::ClampToEdge
            } else {
                ImageAddressMode::Repeat
            }
        };
        let filter = if state.linear {
            ImageFilterMode::Linear
        } else {
            ImageFilterMode::Nearest
        };
        let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: address(state.clamp_u),
            address_mode_v: address(state.clamp_v),
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: ImageFilterMode::Nearest,
            lod_min_clamp: 0.,
            lod_max_clamp: 0.,
            ..default()
        });
        let image = match &loaded.textures[&index].1 {
            duel_models::TextureBytes::Png(png) => Image::from_buffer(
                png,
                ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                false,
                sampler,
                RenderAssetUsages::default(),
            )
            .map_err(|e| format!("Texture decode failed: {e}"))?,
            duel_models::TextureBytes::Rgba {
                width,
                height,
                pixels,
            } => {
                // The bytes are GS texel values, sampled like the PNGs
                // (Rgba8Unorm, no colour conversion).
                let mut image = Image::new(
                    Extent3d {
                        width: *width,
                        height: *height,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    pixels.clone(),
                    TextureFormat::Rgba8Unorm,
                    RenderAssetUsages::default(),
                );
                image.sampler = sampler;
                image
            }
        };
        let handle = images.add(image);
        created.push(handle.id());
        self.cache.insert(key, handle.clone());
        Ok(handle)
    }
}

/// The material of one draw group seen through `ndc` (GS projection and
/// board camera).
pub(crate) fn model_material(
    state: &duel_models::DrawState,
    ndc: Mat4,
    texture: Handle<Image>,
    dither: Mat4,
    gs16: bool,
) -> ModelMaterial {
    let blend = blend_code(state.blend);
    ModelMaterial {
        ndc_from_world: ndc,
        params: Vec4::new(
            f32::from(u8::from(state.textured)),
            state.alpha_greater_than.map_or(-1.0, f32::from),
            f32::from(u8::from(gs16)),
            f32::from(u8::from(state.texture_alpha)),
        ),
        texture,
        dither_rows: dither,
        mode: Vec4::new(
            f32::from(blend),
            f32::from(u8::from(state.clamp_u)),
            f32::from(u8::from(state.clamp_v)),
            f32::from(u8::from(state.linear)),
        ),
        fog: Vec4::ZERO,
        fog_colour: Vec4::ZERO,
        blend,
    }
}

impl ModelMaterial {
    pub(crate) fn fog_state(&self) -> ([f32; 4], [f32; 4]) {
        (self.fog.to_array(), self.fog_colour.to_array())
    }
    /// The arena's height fog (`ArenaLight::fog_uniforms`).
    pub(crate) fn set_fog(&mut self, fog: [f32; 4], colour: [f32; 4]) {
        self.fog = Vec4::from_array(fog);
        self.fog_colour = Vec4::from_array(colour);
    }
    pub(crate) fn set_ndc(&mut self, ndc: Mat4) {
        self.ndc_from_world = ndc;
    }
}

/// [`ModelMaterial::set_ndc`] only when the matrix changes: a material taken
/// mutably is prepared again (its uniform buffer and bind group rebuilt) in
/// the render world, every update for every unit (issue #20).
pub(crate) fn set_ndc_if_changed(materials: &mut Assets<ModelMaterial>, handle: &Handle<ModelMaterial>, ndc: Mat4) {
    if materials.get(handle).is_some_and(|m| m.ndc_from_world != ndc) {
        if let Some(mut material) = materials.get_mut(handle) {
            material.set_ndc(ndc);
        }
    }
}

#[derive(Resource)]
struct ModelRuntime {
    scene: UnitScene,
    meshes: Vec<Handle<Mesh>>,
    view: roses_game::camera::Mat4,
    gs16: bool,
    capture: bool,
}

/// One original update at 25 Hz: draw time advances, meshes are re-skinned.
fn model_tick(mut runtime: ResMut<ModelRuntime>, mut meshes: ResMut<Assets<Mesh>>) {
    runtime.scene.tick();
    match runtime.scene.frame() {
        Ok(frame) => {
            for (handle, mesh) in runtime.meshes.iter().zip(&frame) {
                if let Some(mut target) = meshes.get_mut(handle) {
                    *target = mesh_of(mesh);
                }
            }
        }
        Err(error) => eprintln!("Duel model frame failed: {error}"),
    }
}

fn setup_model(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    mut presents: ResMut<Assets<ModelPresentMaterial>>,
    mut runtime: ResMut<ModelRuntime>,
    capture: Option<ResMut<CaptureState>>,
    bridge: Option<Res<CaptureBridge>>,
    mut exit: MessageWriter<AppExit>,
) {
    let result = (|| -> Result<(), String> {
        let frame = runtime.scene.frame()?;
        let ndc = to_mat4(&duel_board::ndc_from_source(
            &runtime.view,
            &roses_game::camera::identity(),
        ));
        let dither = dither_mat4(&runtime.scene.loaded.dither);
        let mut guard = bridge
            .as_ref()
            .map(|b| b.0.lock().expect("Capture progress mutex poisoned"));
        let mut progress = guard.as_deref_mut();
        let mut textures = ModelTextures::default();
        let mut handles = Vec::new();
        for (order, (group, mesh)) in runtime.scene.groups.iter().zip(&frame).enumerate() {
            let mut created = Vec::new();
            let texture = textures.get(&mut images, &runtime.scene.loaded, group, &mut created)?;
            if let Some(p) = progress.as_deref_mut() {
                p.textures.extend(created);
            }
            let handle = meshes.add(mesh_of(mesh));
            let material = materials.add(model_material(
                &group.state,
                ndc,
                texture,
                dither,
                runtime.gs16,
            ));
            if let Some(p) = progress.as_deref_mut() {
                p.meshes.push(handle.id());
                p.materials.push(material.id().untyped());
            }
            commands.spawn((
                Name::new(format!(
                    "card {} pass {} draw {}",
                    runtime.scene.loaded.model.card, group.pass, group.draw
                )),
                Mesh3d(handle.clone()),
                MeshMaterial3d(material),
                group_transform(order),
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));
            handles.push(handle);
        }
        runtime.meshes = handles;
        // 512x512 GS canvas in byte space (cleared to black like dump draw
        // 6997), then a present pass: 1:1 capture or a 4:3 window.
        let mut canvas = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm, None);
        let filter = if capture.is_some() {
            ImageFilterMode::Nearest
        } else {
            ImageFilterMode::Linear
        };
        canvas.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            mag_filter: filter,
            min_filter: filter,
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            ..default()
        });
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
        let quantize = runtime.capture && runtime.gs16;
        let present = presents.add(ModelPresentMaterial {
            canvas: canvas.clone(),
            params: Vec4::new(f32::from(u8::from(quantize)), 0.0, 0.0, 0.0),
        });
        let quad = meshes.add(Rectangle::new(2., 2.));
        if let Some(p) = progress.as_deref_mut() {
            p.textures.push(canvas.id());
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
                Image::new_target_texture(512, 512, TextureFormat::Rgba8UnormSrgb, None);
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
        eprintln!("Duel model setup failed: {error}");
        exit.write(AppExit::error());
    }
}

/// Capture readiness for this mode's own pipelines (the shared capture
/// driver in main.rs saves the PNG once this reports ready frames).
fn inspect_model_progress(
    bridge: Res<CaptureBridge>,
    cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    textures: Res<RenderAssets<GpuImage>>,
    materials: Res<ErasedRenderAssets<PreparedMaterial>>,
) {
    let mut state = bridge.0.lock().expect("Capture progress mutex poisoned");
    let mut model_pipeline = false;
    let mut own_ready = true;
    let mut all_ready = true;
    for pipeline in cache.pipelines() {
        let label = match &pipeline.descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(d) => d.label.as_deref(),
            _ => None,
        };
        let own = label.is_some_and(is_model_pipeline);
        model_pipeline |= label == Some(PIPELINE);
        if own && !matches!(pipeline.state, CachedPipelineState::Ok(_)) {
            own_ready = false;
        }
        match &pipeline.state {
            CachedPipelineState::Err(
                ShaderCacheError::ShaderNotLoaded(_)
                | ShaderCacheError::ShaderImportNotYetAvailable,
            ) => all_ready = false,
            CachedPipelineState::Err(error) => {
                state.error = Some(format!("GPU pipeline error: {error}"));
                return;
            }
            CachedPipelineState::Ok(_) => (),
            _ => all_ready = false,
        }
    }
    state.scene_ready = model_pipeline
        && own_ready
        && !state.meshes.is_empty()
        && state.meshes.iter().all(|id| meshes.get(*id).is_some())
        && state.textures.iter().all(|id| textures.get(*id).is_some())
        && state
            .materials
            .iter()
            .all(|id| materials.get(*id).is_some());
    all_ready &= state.scene_ready;
    state.ready_frames = if all_ready {
        state.ready_frames.saturating_add(1)
    } else {
        0
    };
}

pub fn run(args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let options = parse(args)?;
    let loaded = duel_models::load_model(&options.manifest, options.card)?;
    let manifest_sha256 = loaded.manifest_sha256.clone();
    let model_sha256 = loaded.model_sha256.clone();
    let mut scene = UnitScene::new(
        loaded,
        options.motion,
        options.cell,
        options.side,
        options.brightness,
    )?;
    if let Some(t) = options.time {
        scene.clock.time = t;
    }
    for _ in 0..options.ticks {
        scene.tick();
    }
    for message in &scene.skipped {
        eprintln!("Duel model: not drawn: {message}");
    }
    let cursor = options.cursor.unwrap_or(options.cell);
    let view = BoardCamera::snapped(cursor)
        .view()
        .ok_or("Board camera view is degenerate")?;
    let frame = scene.frame()?;
    let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
    for mesh in &frame {
        for p in &mesh.positions {
            let s = duel_board::gs_screen(&view, p.map(f64::from));
            for k in 0..2 {
                min[k] = min[k].min(s[k]);
                max[k] = max[k].max(s[k]);
            }
        }
    }
    let model = &scene.loaded.model;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "duel_models_manifest_sha256": manifest_sha256,
            "model_sha256": model_sha256,
            "monster_archive_sha256": scene.loaded.source.monster_archive.sha256,
            "card": model.card,
            "motion": options.motion,
            "motion_length": scene.clock.length,
            "motion_time": scene.clock.time,
            "cell": [options.cell.0, options.cell.1],
            "side": options.side,
            "camera_cursor": [cursor.0, cursor.1],
            "object": {
                "position": scene.object.position,
                "rotation": scene.object.rotation,
                "grow": scene.object.grow,
                "recentre": scene.object.recentre,
            },
            "brightness": options.brightness,
            "frame_buffer": if options.gs16 { "C_16S with DTHE (unit DIMX entry)" } else { "RGB888" },
            "gpu_draws": scene.groups.len(),
            "triangles": frame.iter().map(|m| m.positions.len() / 3).sum::<usize>(),
            "screen_bounds": [min, max],
            "not_drawn": scene.skipped,
            "not_implemented": NOT_IMPLEMENTED,
            "renderer_parity_validated": false,
        }))
        .map_err(|e| e.to_string())?
    );
    if options.validate_only {
        return Ok(());
    }
    let _watchdog = options.capture.as_ref().map(|_| CaptureWatchdog::start(0));
    let runtime = ModelRuntime {
        scene,
        meshes: Vec::new(),
        view,
        gs16: options.gs16,
        capture: options.capture.is_some(),
    };
    let caption = format!(
        "Roses Bevy duel model · card {} motion {} on cell ({},{}) · original pose, skinning and lighting · {NOT_IMPLEMENTED}",
        options.card, options.motion, options.cell.0, options.cell.1
    );
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
        .insert_resource(runtime)
        .add_plugins(plugins)
        .add_plugins(MaterialPlugin::<ModelMaterial>::default())
        .add_plugins(MaterialPlugin::<ModelPresentMaterial>::default())
        // The shared capture observer reads cloud materials; none exist here.
        .init_asset::<crate::cloud_render::CloudMaterial>()
        .add_systems(Startup, setup_model);
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
            .add_systems(Render, inspect_model_progress.after(RenderSystems::Render));
    } else {
        // Original updates run once per two PAL fields (25 Hz, `UPDATE_HZ`).
        app.insert_resource(Time::<Fixed>::from_hz(roses_game::UPDATE_HZ))
            .add_systems(FixedUpdate, model_tick)
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

    fn args(s: &str) -> impl Iterator<Item = OsString> {
        s.split(' ')
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn options_default_to_idle_on_the_leader_cell_and_validate() {
        let o = parse(args("--duel-model m.json --card 392")).unwrap();
        assert_eq!(
            (o.card, o.motion, o.cell, o.side, o.cursor, o.ticks, o.gs16),
            (392, 0, (3, 6), 0, None, 0, true)
        );
        let o = parse(args(
            "--duel-model m.json --card 5 --motion 2 --time 11 --cell 1,2 --side 1 --cursor 2,2 --rgb888",
        ))
        .unwrap();
        assert_eq!(
            (o.motion, o.time, o.cell, o.side, o.cursor, o.gs16),
            (2, Some(11.0), (1, 2), 1, Some((2, 2)), false)
        );
        assert!(parse(args("--duel-model m.json")).is_err());
        assert!(parse(args("--card 3")).is_err());
        assert!(parse(args("--duel-model m.json --card 3 --side 2")).is_err());
        assert!(parse(args("--duel-model m.json --card 3 --cell 7,1")).is_err());
        assert!(parse(args("--duel-model m.json --card 3 --brightness 2")).is_err());
        assert!(parse(args("--duel-model m.json --card 3 --capture x.jpg")).is_err());
        assert!(parse(args("--duel-model m.json --card 3 --bogus")).is_err());
    }

    #[test]
    fn group_sort_keys_are_the_draw_order() {
        // Bevy's key: view z (identity camera) of the entity's affine
        // applied to the mesh AABB centre.
        let key = |order: usize, centre: f32| {
            group_transform(order)
                .compute_affine()
                .transform_point3(Vec3::new(1e5, -1e5, centre))
                .z
        };
        let e = duel_models::DRAW_ORDER_EXTENT;
        for order in 0..duel_models::MAX_DRAW_GROUPS - 1 {
            assert!(key(order, e) < key(order + 1, -e));
            assert_eq!(key(order, e), duel_models::draw_order_key(order, e));
        }
        // The previous translation-only key followed the geometry: a
        // blended group 15 units nearer sorted before an opaque one.
        let old = |order: usize, centre: f32| -100_000.0 + order as f32 + centre;
        assert!(old(7, 144.0) < old(0, 159.5));
        assert!(key(0, 159.5) < key(7, 144.0));
    }

    #[test]
    fn blend_codes_follow_the_alpha_register() {
        assert_eq!(blend_code(None), 0);
        assert_eq!(blend_code(Some(Blend::Normal)), 1);
        assert_eq!(blend_code(Some(Blend::Additive)), 2);
        assert_eq!(blend_code(Some(Blend::Subtractive)), 3);
        assert!(is_model_pipeline(PIPELINE) && !is_model_pipeline("roses-duel-board"));
    }
}
