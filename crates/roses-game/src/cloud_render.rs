//! A bounded GPU raster pass for the researched c400 particles.
//! Bevy owns the graphics device; this pass preserves ordered byte-space blends.
use bevy::{
    asset::RenderAssetUsages,
    camera::{
        RenderTarget,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::{
        render_resource::{
            AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, ShaderType,
            SpecializedMeshPipelineError, TextureDimension, TextureFormat,
        },
        storage::ShaderBuffer,
    },
    shader::ShaderRef,
};
use roses_game::{
    cloud_raster::{CLOUD_DIMX, FADE_DIMX, dither},
    cloud_source::LoadedCloudSource,
    clouds::{CloudQuad, MAX_QUADS, TickInput},
};

pub const PIPELINE: &str = "roses-cloud-raster";
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct CloudSettings {
    rows: [IVec4; 4],
    /// Dither of the opaque navy fade drawn before the clouds.
    fade_rows: [IVec4; 4],
    /// x: navy overlay GS vertex alpha 0..=128 over the (black) world.
    pub fade_alpha: IVec4,
}
fn dither_rows(dimx: u64) -> [IVec4; 4] {
    std::array::from_fn(|row| IVec4::from_array(std::array::from_fn(|col| dither(dimx, col, row))))
}
impl Default for CloudSettings {
    fn default() -> Self {
        Self {
            rows: dither_rows(CLOUD_DIMX),
            fade_rows: dither_rows(FADE_DIMX),
            fade_alpha: IVec4::new(128, 0, 0, 0),
        }
    }
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CloudMaterial {
    #[texture(0)]
    pub atlas: Handle<Image>,
    #[storage(1, read_only)]
    pub vertices: Handle<ShaderBuffer>,
    #[storage(2, read_only)]
    pub bins: Handle<ShaderBuffer>,
    #[uniform(3)]
    pub settings: CloudSettings,
    /// Stonehenge world render (sRGB target) drawn before the navy fade.
    #[texture(4)]
    pub world: Handle<Image>,
}
impl Material for CloudMaterial {
    fn vertex_shader() -> ShaderRef {
        "clouds.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "clouds.wgsl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.label = Some(PIPELINE.into());
        descriptor.vertex.buffers = vec![
            layout
                .0
                .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?,
        ];
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}
#[derive(Resource)]
pub struct CloudAnimation {
    pub source: LoadedCloudSource,
    pub quads: Vec<CloudQuad>,
    pub vertices: Option<Handle<ShaderBuffer>>,
    pub bins: Option<Handle<ShaderBuffer>>,
    pub camera: Option<Entity>,
    pub target: Option<Handle<Image>>,
    pub enabled: bool,
    pub animate: bool,
    pub tick: u32,
    /// Live updates stop here (capture checks of the per-frame upload path).
    pub stop_at: Option<u32>,
    /// Global fade command 2 seen by c400: begin its exit fade.
    pub exit_requested: bool,
    pub material: Option<Handle<CloudMaterial>>,
    /// The ending's c402 runs in place of the history's c400, whose state
    /// (with its tick and quads) is kept here to come back afterwards.
    pub converging: Option<(roses_game::clouds::CloudState, u32, Vec<CloudQuad>)>,
}
impl CloudAnimation {
    pub fn new(source: LoadedCloudSource, ticks: u32, animate: bool) -> Result<Self, String> {
        if source.emission_checkpoint && (ticks != 0 || animate) {
            return Err(
                "An emission-only checkpoint cannot be advanced; use the saved update checkpoint"
                    .into(),
            );
        }
        let resume = !source.emission_checkpoint;
        let quads = source.state.emit(&source.sine);
        let mut this = Self {
            source,
            quads,
            vertices: None,
            bins: None,
            camera: None,
            target: None,
            enabled: true,
            animate,
            tick: 0,
            stop_at: None,
            exit_requested: false,
            material: None,
            converging: None,
        };
        if resume {
            this.step()?;
        }
        for _ in 0..ticks {
            this.step()?;
        }
        Ok(this)
    }
    /// Launch c402 (`0x1F5B00(0, 402)`): a fresh cloud state initialized
    /// from the shared RNG as it stands (the original's rand state at that
    /// moment depends on the whole session [H]), drawn over the story world.
    pub fn start_converging(&mut self) -> Result<(), String> {
        if self.converging.is_some() {
            return Ok(());
        }
        let arithmetic = self.source.state.arithmetic();
        let state = roses_game::clouds::CloudState::initialize_converging(
            &mut self.source.rng,
            arithmetic,
        )?;
        let kept = std::mem::replace(&mut self.source.state, state);
        self.converging = Some((kept, self.tick, std::mem::take(&mut self.quads)));
        Ok(())
    }
    /// c402 ended (the story moved on): the history's c400 comes back as it
    /// was.
    pub fn stop_converging(&mut self) {
        if let Some((state, tick, quads)) = self.converging.take() {
            self.source.state = state;
            self.tick = tick;
            self.quads = quads;
        }
    }
    pub fn settled(&self) -> bool {
        self.stop_at.is_some_and(|stop| self.tick >= stop)
    }
    fn step(&mut self) -> Result<(), String> {
        self.tick = self.tick.wrapping_add(1);
        if self.source.state.complete() {
            // c400 returned completion code 8; the effect is no longer drawn.
            self.quads.clear();
            return Ok(());
        }
        // Global main-loop calls are explicit context outside the particle module.
        for _ in 0..self.source.external_rng_calls_per_tick {
            self.source.rng.next_u31();
        }
        let frame = self.source.state.step(
            &mut self.source.rng,
            &self.source.sine,
            TickInput {
                request_exit: self.exit_requested,
            },
        )?;
        self.quads = frame.quads;
        Ok(())
    }
}
#[derive(Debug, serde::Serialize)]
pub struct BandComparison {
    pub cloud_tick: u32,
    pub cloud_counter: u32,
    pub quads: usize,
    pub pixels: usize,
    pub differing_pixels: usize,
    pub max_channel_error: u8,
    /// Tile-aligned differences are the signature of stale per-tile bins.
    pub differing_16px_tiles: usize,
}
/// Compare a 512x512 RGBA capture's cloud band with the CPU model raster of
/// the quads currently uploaded. This verifies the native GPU path only.
pub fn compare_band_with_cpu(
    animation: &CloudAnimation,
    fade_alpha: u8,
    rgba: &[u8],
) -> Result<BandComparison, String> {
    use roses_game::cloud_raster::{
        BAND_BOTTOM, BAND_TOP, CLOUD_DIMX, WIDTH, fade_band_alpha, raster_band_over,
    };
    if rgba.len() != WIDTH * WIDTH * 4 {
        return Err("Cloud band comparison requires a 512x512 RGBA capture".into());
    }
    let cpu = raster_band_over(
        &animation.quads,
        &animation.source.rgba_gs,
        CLOUD_DIMX,
        &fade_band_alpha(fade_alpha),
    );
    let mut report = BandComparison {
        cloud_tick: animation.tick,
        cloud_counter: animation.source.state.counter(),
        quads: animation.quads.len(),
        pixels: WIDTH * (BAND_BOTTOM - BAND_TOP),
        differing_pixels: 0,
        max_channel_error: 0,
        differing_16px_tiles: 0,
    };
    let mut tiles = std::collections::BTreeSet::new();
    for y in BAND_TOP..BAND_BOTTOM {
        for x in 0..WIDTH {
            let gpu = &rgba[(y * WIDTH + x) * 4..][..3];
            let cpu = &cpu[((y - BAND_TOP) * WIDTH + x) * 3..][..3];
            let error = gpu
                .iter()
                .zip(cpu)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            if error != 0 {
                report.differing_pixels += 1;
                report.max_channel_error = report.max_channel_error.max(error);
                tiles.insert((x / 16, y / 16));
            }
        }
    }
    report.differing_16px_tiles = tiles.len();
    Ok(report)
}

/// Visible 16x16 tiles: 32 columns by rows 9..23 of the 144..368 band.
const VISIBLE_TILES: usize = 32 * 14;
/// Fixed storage sizes. Bevy reuses a `ShaderBuffer`'s GPU buffer only when
/// its byte size is unchanged; a resized upload creates a new buffer while the
/// material bind group keeps the old one, freezing stale tile lists on screen.
pub const VERTEX_RECORDS: usize = MAX_QUADS * 5;
pub const BIN_WORDS: usize = 2048 + MAX_QUADS * VISIBLE_TILES;

pub fn packed(quads: &[CloudQuad]) -> Result<(Vec<[f32; 4]>, Vec<u32>), String> {
    if quads.len() > MAX_QUADS {
        return Err(format!(
            "Cloud frame has {} quads; maximum is {MAX_QUADS}",
            quads.len()
        ));
    }
    let mut vertices = Vec::with_capacity(VERTEX_RECORDS);
    let mut tiles: Vec<Vec<u32>> = (0..1024).map(|_| Vec::new()).collect();
    for (index, quad) in quads.iter().enumerate() {
        for i in 0..4 {
            vertices.push([
                quad.xy[i][0] as f32,
                quad.xy[i][1] as f32,
                quad.uv[i][0] as f32 / 16.0,
                quad.uv[i][1] as f32 / 16.0,
            ]);
        }
        vertices.push(quad.rgba.map(f32::from));
        let minx = quad.xy.iter().map(|p| p[0]).min().unwrap();
        let maxx = quad.xy.iter().map(|p| p[0]).max().unwrap();
        let miny = quad.xy.iter().map(|p| p[1]).min().unwrap();
        let maxy = quad.xy.iter().map(|p| p[1]).max().unwrap();
        if maxx <= 0 || maxy <= 144 || minx >= 512 || miny >= 368 {
            continue;
        }
        for y in miny.max(144) / 16..=maxy.min(367) / 16 {
            for x in minx.max(0) / 16..=maxx.min(511) / 16 {
                tiles[(y * 32 + x) as usize].push(index as u32);
            }
        }
    }
    // Padding is never indexed: bins reference only supplied quads.
    vertices.resize(VERTEX_RECORDS, [0.; 4]);
    let mut bins = Vec::with_capacity(BIN_WORDS);
    bins.resize(2048, 0);
    for (i, tile) in tiles.into_iter().enumerate() {
        bins[2 * i] = bins.len() as u32;
        bins[2 * i + 1] = tile.len() as u32;
        bins.extend(tile);
    }
    // Each quad lands in at most every visible tile once, so this cannot fail.
    debug_assert!(bins.len() <= BIN_WORDS);
    bins.resize(BIN_WORDS, 0);
    Ok((vertices, bins))
}
pub fn setup_clouds(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    buffers: &mut Assets<ShaderBuffer>,
    materials: &mut Assets<CloudMaterial>,
    animation: &mut CloudAnimation,
) -> Result<
    (
        Handle<Mesh>,
        Handle<CloudMaterial>,
        Handle<Image>,
        Handle<Image>,
    ),
    String,
> {
    let atlas = images.add(Image::new(
        Extent3d {
            width: 128,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        animation.source.rgba_gs.clone(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    ));
    let (vertices, bins) = packed(&animation.quads)?;
    let vertices = buffers.add(ShaderBuffer::from(vertices));
    let bins = buffers.add(ShaderBuffer::from(bins));
    animation.vertices = Some(vertices.clone());
    animation.bins = Some(bins.clone());
    let material = materials.add(CloudMaterial {
        atlas: atlas.clone(),
        vertices,
        bins,
        settings: CloudSettings::default(),
        // Replaced by the world target once it exists.
        world: atlas.clone(),
    });
    animation.material = Some(material.clone());
    let mesh = meshes.add(Rectangle::new(2., 2.));
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::IDENTITY,
        NoFrustumCulling,
        RenderLayers::layer(2),
    ));
    let mut target = Image::new_target_texture(512, 512, TextureFormat::Rgba8UnormSrgb, None);
    target.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    let target = images.add(target);
    animation.target = Some(target.clone());
    animation.camera = Some(
        commands
            .spawn((
                Camera3d::default(),
                Camera {
                    // After the world (-1): the fade may reveal it this frame.
                    order: 0,
                    is_active: animation.enabled,
                    ..default()
                },
                RenderTarget::Image(target.clone().into()),
                Transform::IDENTITY,
                Tonemapping::None,
                DebandDither::Disabled,
                Msaa::Off,
                RenderLayers::layer(2),
            ))
            .id(),
    );
    Ok((mesh, material, atlas, target))
}
pub fn animate_clouds(
    animation: Option<ResMut<CloudAnimation>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut animation) = animation else {
        return;
    };
    if !animation.animate || animation.settled() {
        return;
    }
    // The clock keeps counting off-stage so live-tick captures still settle;
    // the particle state itself only advances while its scene is shown.
    if !animation.enabled {
        animation.tick = animation.tick.wrapping_add(1);
        return;
    }
    if let Err(error) = animation.step() {
        eprintln!("Cloud update failed: {error}");
        exit.write(AppExit::error());
        return;
    }
    let (vertices, bins) = match packed(&animation.quads) {
        Ok(packed) => packed,
        Err(error) => {
            eprintln!("Cloud upload failed: {error}");
            exit.write(AppExit::error());
            return;
        }
    };
    if let Some(mut buffer) = animation.vertices.as_ref().and_then(|h| buffers.get_mut(h)) {
        buffer.set_data(vertices);
    }
    if let Some(mut buffer) = animation.bins.as_ref().and_then(|h| buffers.get_mut(h)) {
        buffer.set_data(bins);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(xy: [[i32; 2]; 4], marker: u8) -> CloudQuad {
        CloudQuad {
            group: 0,
            particle: marker,
            age: 3,
            xy,
            uv: [[0, 0], [512, 0], [0, 512], [512, 512]],
            rgba: [marker, 128, 255, 64],
        }
    }

    fn tile(bins: &[u32], x: usize, y: usize) -> &[u32] {
        let slot = (y * 32 + x) * 2;
        let start = bins[slot] as usize;
        let end = start.checked_add(bins[slot + 1] as usize).unwrap();
        assert!(start >= 2048 && end <= bins.len());
        &bins[start..end]
    }

    fn validate_bins(vertices: &[[f32; 4]], bins: &[u32], quads: usize) {
        assert!(bins.len() >= 2048);
        for y in 0..32 {
            for x in 0..32 {
                let indices = tile(bins, x, y);
                for &index in indices {
                    assert!((index as usize) < quads);
                    assert!((index as usize) * 5 + 4 < vertices.len());
                }
                assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
                if !(9..23).contains(&y) {
                    assert!(indices.is_empty(), "non-visible tile {x},{y}");
                }
            }
        }
    }

    #[test]
    fn overlapping_quads_preserve_input_order_and_vertex_records() {
        let bounds = [[8, 152], [40, 152], [8, 180], [40, 180]];
        let quads = [quad(bounds, 17), quad(bounds, 2), quad(bounds, 91)];
        let (vertices, bins) = packed(&quads).unwrap();
        validate_bins(&vertices, &bins, quads.len());
        assert_eq!(tile(&bins, 1, 10), [0, 1, 2]);
        assert_eq!((vertices.len(), bins.len()), (VERTEX_RECORDS, BIN_WORDS));
        assert!(vertices[15..].iter().all(|v| *v == [0.; 4]));
        // Blend order follows the supplied sequence, not particle ID or color.
        assert_eq!(vertices[4], [17., 128., 255., 64.]);
        assert_eq!(vertices[9], [2., 128., 255., 64.]);
        assert_eq!(vertices[14], [91., 128., 255., 64.]);
        assert_eq!(vertices[0], [8., 152., 0., 0.]);
        assert_eq!(vertices[3], [40., 180., 32., 32.]);
    }

    #[test]
    fn clipped_outside_and_empty_quads_never_create_invalid_indices() {
        let quads = [
            quad([[-20, 150], [0, 150], [-20, 170], [0, 170]], 0),
            quad([[512, 150], [530, 150], [512, 170], [530, 170]], 1),
            quad([[20, 120], [40, 120], [20, 144], [40, 144]], 2),
            quad([[20, 368], [40, 368], [20, 400], [40, 400]], 3),
            quad([[-10, 140], [10, 140], [-10, 155], [10, 155]], 4),
            quad([[503, 360], [530, 360], [503, 380], [530, 380]], 5),
        ];
        let (vertices, bins) = packed(&quads).unwrap();
        validate_bins(&vertices, &bins, quads.len());
        assert_eq!(tile(&bins, 0, 9), [4]);
        assert_eq!(tile(&bins, 31, 22), [5]);
        assert_eq!(bins[2048..2050], [4, 5]);
        assert!(bins[2050..].iter().all(|&i| i == 0));
        let (vertices, bins) = packed(&[]).unwrap();
        assert_eq!((vertices.len(), bins.len()), (VERTEX_RECORDS, BIN_WORDS));
        assert!(bins[1..2048].iter().step_by(2).all(|&count| count == 0));
        validate_bins(&vertices, &bins, 0);
    }

    #[test]
    fn upload_sizes_never_change_between_frames() {
        // A resized ShaderBuffer gets a new GPU buffer that the material bind
        // group never sees; live clouds then drew stale 16px tile lists.
        let full = quad([[-400, -400], [900, -400], [-400, 900], [900, 900]], 1);
        let worst = vec![full; MAX_QUADS];
        let (vertices, bins) = packed(&worst).unwrap();
        assert_eq!((vertices.len(), bins.len()), (VERTEX_RECORDS, BIN_WORDS));
        assert_eq!(tile(&bins, 31, 22).len(), MAX_QUADS);
        let (vertices, bins) = packed(&worst[..1]).unwrap();
        assert_eq!((vertices.len(), bins.len()), (VERTEX_RECORDS, BIN_WORDS));
        assert!(packed(&vec![worst[0].clone(); MAX_QUADS + 1]).is_err());
    }

    #[test]
    fn triangles_crossing_tile_boundaries_are_not_falsely_culled() {
        // Rotated strip crosses x=0/16/32 and y=144/160/176. Its vertices
        // alone do not describe every tile crossed by the two triangles.
        let source = quad([[16, 143], [33, 160], [-1, 160], [16, 177]], 0);
        let (vertices, bins) = packed(std::slice::from_ref(&source)).unwrap();
        validate_bins(&vertices, &bins, 1);
        let edge = |a: [i64; 2], b: [i64; 2], p: [i64; 2]| {
            (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
        };
        let points = source.xy.map(|p| p.map(|v| i64::from(v) * 2));
        let mut covered = 0;
        for y in 144..190 {
            for x in 0..40 {
                let p = [x * 2 + 1, y * 2 + 1];
                let inside = [[0, 1, 2], [2, 1, 3]].iter().any(|&[a, b, c]| {
                    let weights = [
                        edge(points[a], points[b], p),
                        edge(points[b], points[c], p),
                        edge(points[c], points[a], p),
                    ];
                    weights.iter().all(|&w| w >= 0) || weights.iter().all(|&w| w <= 0)
                });
                if inside {
                    covered += 1;
                    assert!(
                        tile(&bins, x as usize / 16, y as usize / 16).contains(&0),
                        "covered pixel missing from tile: {x},{y}"
                    );
                }
            }
        }
        assert!(
            covered > 500,
            "coverage oracle must exercise the strip interior"
        );
    }
}
