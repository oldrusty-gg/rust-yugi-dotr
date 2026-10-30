// Duel board model draw: original raw int16 positions, a CPU-built matrix that
// already contains the GS projection, the native board camera and the model
// placement (see duel_board.rs ndc_from_source), and GS byte arithmetic. The
// target is a Rgba8Unorm canvas, so values are written as bytes / 255.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ndc_from_source: mat4x4<f32>;
// Vertex colour in GS units (RGB 0..255, A with 128 = 1.0); constant per draw.
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> color_gs: vec4<f32>;
// x: PRIM TME (1 textured), y: ATST GREATER reference (-1 = alpha test off),
// z: 1 = write through the 16-bit C_16S frame buffer with DTHE, w: TEX0 TCC.
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var source_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var source_sampler: sampler;
// DIMX from the executable's table: column k holds row k (y & 3), x & 3 indexes it.
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> dither_rows: mat4x4<f32>;
// Cursor tint pass: RGB added by the full-screen sprite (bytes), w = 1 when on.
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> tint: vec4<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let c = ndc_from_source * vec4<f32>(input.position, 1.0);
    var out: VertexOutput;
    // Depth 1/w keeps the GS order (Z24 = 16779264/w - 2048.25, ZTST GREATER);
    // fixed-point Z and the VU clipper are not reproduced. Near plane w = 1.
    out.position = vec4<f32>(c.x, c.y, 1.0, c.w);
    out.uv = input.uv;
    return out;
}

// 16-bit write: add the dither value, clamp (COLCLAMP), keep 5 bits.
fn write16(value: vec3<f32>, d: f32) -> vec3<f32> {
    return floor(clamp(value + vec3<f32>(d), vec3<f32>(0.0), vec3<f32>(255.0)) / 8.0) * 8.0;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    var rgb = color_gs.rgb;
    if params.x > 0.5 {
        // Fixed mip 0 (the board draws use MMIN linear, no mipmaps).
        let tex = textureSampleLevel(source_texture, source_sampler, input.uv, 0.0);
        // TFX MODULATE: (Ct * Cv) >> 7, each step quantized to bytes.
        let texel = trunc(tex.rgb * 255.0 + vec3<f32>(0.05));
        rgb = min(trunc(texel * color_gs.rgb / 128.0), vec3<f32>(255.0));
        if params.y >= 0.0 {
            // PNG alpha 255 is GS alpha 128 (TEXA TA1); filtered like colour.
            let texel_alpha = select(128.0, trunc(tex.a * 128.0 + 0.05), params.w > 0.5);
            let alpha = min(trunc(texel_alpha * color_gs.a / 128.0), 255.0);
            // ATST GREATER with AFAIL KEEP: failing pixels write neither colour nor Z.
            if alpha <= params.y {
                discard;
            }
        }
    }
    // Fog: every board draw emits F = 255, which leaves the colour unchanged.
    if params.z > 0.5 {
        let p = vec2<u32>(input.position.xy);
        let d = dither_rows[p.y & 3u][p.x & 3u];
        rgb = write16(rgb, d);
        if tint.w > 0.5 {
            // The cell drawn into 0x2000 (same DIMX), the tint sprite added
            // with Cd + Cs*As and dithered again; the composite copies it.
            rgb = write16(rgb + tint.rgb, d);
        }
    } else if tint.w > 0.5 {
        rgb = min(rgb + tint.rgb, vec3<f32>(255.0));
    }
    return vec4<f32>(rgb / 255.0, 1.0);
}
