// Campaign map model draw: original raw int16 positions, a CPU-built matrix
// that already contains the GS projection, the traced map camera and the model
// placement (see campaign_map.rs), per-vertex GS colours from the lit material,
// and GS byte arithmetic. The target is a Rgba8Unorm canvas, so values are
// written as bytes / 255; the alpha output is As (GS 128 = 1.0) for the
// fixed-function blend of PRIM ABE strips.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ndc_from_source: mat4x4<f32>;
// x: PRIM TME (1 textured), y: ATST GREATER reference (-1 = alpha test off),
// z: 1 = write through the 16-bit C_16S frame buffer with DTHE, w: TEX0 TCC.
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var source_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var source_sampler: sampler;
// DIMX from the executable's table: column k holds row k (y & 3), x & 3 indexes it.
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> dither_rows: mat4x4<f32>;
// x: extra alpha factor (dot fade-in), y: 1 = force the alpha output on,
// z: 1 - the arena stage light g (0x2DF5B2 / 128; 0 = full).
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> extra: vec4<f32>;
// Height fog (battle arena stage only): x Q0, y Q1, z 1 = on; FOGCOL RGB
// bytes; the model-to-world Y row.
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> fog: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> fog_colour: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var<uniform> world_y: vec4<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) fog_f: f32,
};

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let c = ndc_from_source * vec4<f32>(input.position, 1.0);
    var out: VertexOutput;
    // Depth 1/w keeps the GS order (Z24 = 16779264/w - 2048.25, ZTST GREATER);
    // fixed-point Z and the VU clipper are not reproduced. Near plane w = 1.
    out.position = vec4<f32>(c.x, c.y, 1.0, c.w);
    out.uv = input.uv;
    out.color = input.color;
    out.fog_f = 255.0;
    if fog.z > 0.5 {
        let y = dot(world_y.xyz, input.position) + world_y.w;
        out.fog_f = floor(max(0.0, 255.0 + min(0.0, 255.0 * (fog.x - y) / (fog.y - fog.x))));
    }
    return out;
}

// 16-bit write: add the dither value, clamp (COLCLAMP), keep 5 bits.
fn write16(value: vec3<f32>, d: f32) -> vec3<f32> {
    return floor(clamp(value + vec3<f32>(d), vec3<f32>(0.0), vec3<f32>(255.0)) / 8.0) * 8.0;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    // The stage light scales the (linear) lighting result [H: before the
    // byte conversion of the vertex colour].
    let lit = floor(input.color.rgb * (1.0 - extra.z) + vec3<f32>(0.5 * extra.z));
    var rgb = lit;
    var alpha = input.color.a * extra.x;
    if params.x > 0.5 {
        // Fixed mip 0 (the map draws use MMIN linear, no mipmaps).
        let tex = textureSampleLevel(source_texture, source_sampler, input.uv, 0.0);
        // TFX MODULATE: (Ct * Cv) >> 7, each step quantized to bytes.
        let texel = trunc(tex.rgb * 255.0 + vec3<f32>(0.05));
        rgb = min(trunc(texel * lit / 128.0), vec3<f32>(255.0));
        // PNG alpha 255 is GS alpha 128 (TEXA TA1 / CLUT alpha); filtered like colour.
        let texel_alpha = select(128.0, trunc(tex.a * 128.0 + 0.05), params.w > 0.5);
        alpha = min(trunc(texel_alpha * alpha / 128.0), 255.0);
        if params.y >= 0.0 {
            // ATST GREATER with AFAIL KEEP: failing pixels write neither colour nor Z.
            if alpha <= params.y {
                discard;
            }
        }
    }
    // Fog: every map draw emits F = 255 (unchanged); the arena stage's
    // height fog blends toward FOGCOL [H: PCSX2's integer form].
    if fog.z > 0.5 {
        let f = floor(input.fog_f);
        // FOGCOL goes with the stage light [emu module 373: (192, 224, 255)
        // at g 32 is (48, 56, 63)].
        let fc = floor(fog_colour.rgb * (1.0 - extra.z));
        rgb = floor((rgb - fc) * f / 256.0) + fc;
    }
    if params.z > 0.5 {
        let p = vec2<u32>(input.position.xy);
        let d = dither_rows[p.y & 3u][p.x & 3u];
        rgb = write16(rgb, d);
    }
    return vec4<f32>(rgb / 255.0, clamp(alpha / 128.0, 0.0, 1.0));
}
