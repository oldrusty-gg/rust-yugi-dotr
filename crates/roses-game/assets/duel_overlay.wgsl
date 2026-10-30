// Duel board cell and unit layers (dump draws 6990-6996), drawn after the
// board into the same Rgba8Unorm canvas and depth buffer.
// kind 0: cell fill, world quad, additive Cd + Cs*As/128, Z GEQUAL, no Z write.
// kind 1: antialiased line band, pre-projected (x, y NDC, depth 1/w), uv.x is
//         the signed minor-axis distance in pixels; additive.
// kind 2: textured unit quad (card back, emblem), normal alpha blend, Z write.
// kind 3: textured quad written as Cs * As (ALPHA (Cs - 0) * As + 0, the
//         battle scene's damage digits), replacing the destination.
// kind 4: textured quad added as Cs * As + Cd (the return's flash layers).
// kind 5: untextured quad subtracted, Cd - Cs * As (the arena crossfade).
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ndc_from_source: mat4x4<f32>;
// GS vertex colour: RGB bytes, A with 128 = 1.0.
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> color_gs: vec4<f32>;
// x: kind, y: 1 = 16-bit C_16S write with DTHE.
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> params: vec4<f32>;
// DIMX: column k holds row k (y & 3).
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> dither_rows: mat4x4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var source_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var source_sampler: sampler;

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
    var out: VertexOutput;
    if params.x > 0.5 && params.x < 1.5 {
        out.position = vec4<f32>(input.position.xy, input.position.z, 1.0);
    } else {
        let c = ndc_from_source * vec4<f32>(input.position, 1.0);
        // params.z = 1: in front of everything (depth 1), e.g. the battle
        // scene's 3D cards drawn over the board (Z 0xFFFFFF).
        out.position = vec4<f32>(c.x, c.y, select(1.0, c.w, params.z > 0.5), c.w);
    }
    out.uv = input.uv;
    return out;
}

fn write16(value: vec3<f32>, d: f32) -> vec3<f32> {
    return floor(clamp(value + vec3<f32>(d), vec3<f32>(0.0), vec3<f32>(255.0)) / 8.0) * 8.0;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let p = vec2<u32>(input.position.xy);
    let d = dither_rows[p.y & 3u][p.x & 3u];
    let gs16 = params.y > 0.5;
    if params.x < 1.5 {
        var alpha = color_gs.a;
        if params.x > 0.5 {
            // AA1: coverage replaces the alpha of the two pixels across the line.
            alpha = floor(max(0.0, 1.0 - abs(input.uv.x)) * color_gs.a);
        }
        // ATST GREATER 0.
        if alpha <= 0.0 {
            discard;
        }
        // Additive: the destination is already a 5-bit value (multiple of 8),
        // so dithering the source term gives the GS result; saturation above
        // 248 is restored by the capture's final 5-bit pass.
        let add = floor(color_gs.rgb * alpha / 128.0);
        let rgb = select(add, write16(add, d), gs16);
        return vec4<f32>(rgb / 255.0, 1.0);
    }
    // Textured quad: MODULATE, TCC alpha (PNG 255 = GS 128), ATST GREATER 0.
    let tex = textureSampleLevel(source_texture, source_sampler, input.uv, 0.0);
    if params.x > 4.5 {
        let o = trunc(color_gs.rgb * color_gs.a / 128.0);
        return vec4<f32>(min(o, vec3<f32>(255.0)) / 255.0, 1.0);
    }
    if params.x > 3.5 {
        let t = trunc(tex.rgb * 255.0 + vec3<f32>(0.05));
        let c = min(trunc(t * color_gs.rgb / 128.0), vec3<f32>(255.0));
        let a = min(trunc(trunc(tex.a * 128.0 + 0.05) * color_gs.a / 128.0), 255.0);
        if a <= 0.0 {
            discard;
        }
        let o = trunc(c * a / 128.0);
        return vec4<f32>(select(o, write16(o, d), gs16) / 255.0, 1.0);
    }
    if params.x > 2.5 {
        let t = trunc(tex.rgb * 255.0 + vec3<f32>(0.05));
        let c = min(trunc(t * color_gs.rgb / 128.0), vec3<f32>(255.0));
        let a = min(trunc(trunc(tex.a * 128.0 + 0.05) * color_gs.a / 128.0), 255.0);
        if a <= 0.0 {
            discard;
        }
        let o = min(trunc(c * a / 128.0), vec3<f32>(255.0));
        return vec4<f32>(select(o, write16(o, d), gs16) / 255.0, 1.0);
    }
    let texel = trunc(tex.rgb * 255.0 + vec3<f32>(0.05));
    let rgb = min(trunc(texel * color_gs.rgb / 128.0), vec3<f32>(255.0));
    let alpha = min(trunc(trunc(tex.a * 128.0 + 0.05) * color_gs.a / 128.0), 255.0);
    if alpha <= 0.0 {
        discard;
    }
    // (Cs - Cd) * As + Cd by the fixed-function blend: exact for opaque texels;
    // partially covered bilinear edges round once instead of per GS step.
    let out = select(rgb, write16(rgb, d), gs16);
    return vec4<f32>(out / 255.0, min(alpha / 128.0, 1.0));
}
