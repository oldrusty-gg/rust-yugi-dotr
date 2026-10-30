// Duel unit model draw (duel_model_render.rs): world positions skinned on the
// CPU, the per-vertex original lighting as GS colours, and a CPU-built matrix
// holding the GS projection and the native board camera (duel_board.rs
// ndc_from_source). The target is a Rgba8Unorm canvas in GS byte values.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ndc_from_world: mat4x4<f32>;
// x: PRIM TME, y: ATST GREATER reference (-1 = alpha test off),
// z: 1 = write through the 16-bit C_16S frame buffer with DTHE, w: TEX0 TCC.
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var source_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var source_sampler: sampler;
// DIMX (unit entry): column k holds row k (y & 3), x & 3 indexes it.
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> dither_rows: mat4x4<f32>;
// x: 0 opaque (ABE 0), 1 ALPHA 0x44, 2 ALPHA 0x48, 3 ALPHA 0x42;
// y / z: CLAMP WMS / WMT clamp (else repeat); w: TEX1 bilinear.
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> mode: vec4<f32>;
// Height fog (the battle arena): x Q0, y Q1, z 1 = on; FOGCOL RGB bytes.
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> fog: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> fog_colour: vec4<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // GS vertex colour: RGB 0..255, A with 128 = 1.0.
    @location(2) color: vec4<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // S/T are perspective-correct (STQ); colours are affine like the GS.
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(linear) color: vec4<f32>,
    // GS fog coefficient F (255 = no fog), interpolated per vertex.
    @location(2) @interpolate(linear) fog_f: f32,
};

// VU1: F = max(0, min(255, depth) + min(0, 255 (Q0 - Y) / (Q1 - Q0))), the
// depth term being 255 in the arena [F ARENA-ENVIRONMENT.md 2].
fn fog_coefficient(y: f32) -> f32 {
    if fog.z < 0.5 {
        return 255.0;
    }
    return floor(max(0.0, 255.0 + min(0.0, 255.0 * (fog.x - y) / (fog.y - fog.x))));
}

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let c = ndc_from_world * vec4<f32>(input.position, 1.0);
    // GS primitive coordinates are 12.4 fixed point (the VU's FTOI4
    // truncates): snap X and Y to 1/16 pixel before rasterising.
    let x = floor(((c.x / c.w) * 256.0 + 2047.5) * 16.0) / 16.0;
    let y = floor((2047.5 - (c.y / c.w) * 256.0) * 16.0) / 16.0;
    var out: VertexOutput;
    // Depth 1/w keeps the GS order (ZTST GREATER on Z24 = 16779264/w - 2048.25).
    out.position = vec4<f32>((x - 2047.5) / 256.0 * c.w, (2047.5 - y) / 256.0 * c.w, 1.0, c.w);
    out.uv = input.uv;
    out.color = input.color;
    out.fog_f = fog_coefficient(input.position.y);
    return out;
}

fn wrap(value: i32, size: i32, clamp_mode: bool) -> i32 {
    if clamp_mode {
        return clamp(value, 0, size - 1);
    }
    return ((value % size) + size) % size;
}

fn texel(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    let p = vec2<i32>(wrap(at.x, size.x, mode.y > 0.5), wrap(at.y, size.y, mode.z > 0.5));
    return round(textureLoad(source_texture, p, 0) * 255.0);
}

// GS texture lookup: 12.4 fixed-point texel coordinates, bilinear with the
// 4-bit fractions around (u - 0.5, v - 0.5), else the nearest texel.
fn gs_sample(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(source_texture));
    var u = vec2<i32>(floor(uv * vec2<f32>(size) * 16.0));
    if mode.w < 0.5 {
        return texel(u >> vec2<u32>(4u), size);
    }
    u = u - vec2<i32>(8);
    let i = u >> vec2<u32>(4u);
    let f = vec2<f32>(u & vec2<i32>(15));
    let t00 = texel(i, size);
    let t10 = texel(i + vec2<i32>(1, 0), size);
    let t01 = texel(i + vec2<i32>(0, 1), size);
    let t11 = texel(i + vec2<i32>(1, 1), size);
    let top = t00 * (16.0 - f.x) + t10 * f.x;
    let bottom = t01 * (16.0 - f.x) + t11 * f.x;
    return trunc((top * (16.0 - f.y) + bottom * f.y) / 256.0);
}

// 16-bit write: add the dither value, clamp (COLCLAMP), keep 5 bits.
fn write16(value: vec3<f32>, d: f32) -> vec3<f32> {
    return floor(clamp(value + vec3<f32>(d), vec3<f32>(0.0), vec3<f32>(255.0)) / 8.0) * 8.0;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    // Gouraud colours: interpolated linearly in screen space (not
    // perspective-corrected) and rounded to bytes (best match to the dump).
    let vertex_color = round(input.color);
    var rgb = vertex_color.rgb;
    var alpha = vertex_color.a;
    if params.x > 0.5 {
        // PNG bytes are GS texel bytes (alpha 128 = 1.0).
        let t = gs_sample(input.uv);
        // TFX MODULATE: (Ct * Cv) >> 7; TCC 1: (At * Av) >> 7, else Av.
        rgb = min(trunc(t.rgb * vertex_color.rgb / 128.0), vec3<f32>(255.0));
        if params.w > 0.5 {
            alpha = min(trunc(t.a * vertex_color.a / 128.0), 255.0);
        }
    }
    // ATST GREATER with AFAIL KEEP: failing pixels write neither colour nor Z.
    if params.y >= 0.0 && alpha <= params.y {
        discard;
    }
    if fog.z > 0.5 {
        // GS fog after the texture modulate: ((C - FOGCOL) F >> 8) + FOGCOL
        // [H: PCSX2's integer form].
        let f = floor(input.fog_f);
        rgb = floor((rgb - fog_colour.rgb) * f / 256.0) + fog_colour.rgb;
    }
    let blend = mode.x;
    if blend < 0.5 {
        if params.z > 0.5 {
            let p = vec2<u32>(input.position.xy);
            rgb = write16(rgb, dither_rows[p.y & 3u][p.x & 3u]);
        }
        return vec4<f32>(rgb / 255.0, 1.0);
    }
    if blend < 1.5 {
        // (Cs - Cd) As + Cd with SrcAlpha / OneMinusSrcAlpha (As <= 128).
        return vec4<f32>(rgb / 255.0, min(alpha / 128.0, 1.0));
    }
    // Additive (One, One) and subtractive (reverse subtract): Cs * As.
    return vec4<f32>(min(trunc(rgb * alpha / 128.0), vec3<f32>(255.0)) / 255.0, 1.0);
}
