// Presents the Rgba8Unorm unit-model canvas (GS byte values) on an sRGB target.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var canvas_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var canvas_sampler: sampler;
// x: 1 = final 5-bit pass of the 16-bit frame buffer (captures).
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> params: vec4<f32>;

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
    out.position = vec4<f32>(input.position.xy, 0.5, 1.0);
    out.uv = input.uv;
    return out;
}

fn display_byte_to_linear(value: vec3<f32>) -> vec3<f32> {
    return select(pow((value + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), value / 12.92, value <= vec3<f32>(0.04045));
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    var bytes = round(textureSampleLevel(canvas_texture, canvas_sampler, input.uv, 0.0).rgb * 255.0);
    if params.x > 0.5 {
        bytes = floor(bytes / 8.0) * 8.0;
    }
    // The present camera's target is sRGB: the inverse transfer keeps bytes.
    return vec4<f32>(display_byte_to_linear(bytes / 255.0), 1.0);
}
