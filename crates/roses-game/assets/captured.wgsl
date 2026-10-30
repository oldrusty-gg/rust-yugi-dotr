// Original source positions and a captured column-major GS homogeneous matrix.
// This shader uses Bevy's material pipeline and render device, not a second GPU.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> clip_from_source_gs: mat4x4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var source_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var source_sampler: sampler;
// x = 1: a blended second-chain draw (GS ALPHA 0x44, As = texel alpha x vertex alpha / 128).
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> params: vec4<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color_gs: vec4<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(linear) color_gs: vec4<f32>,
    @location(2) @interpolate(linear) raw_y: f32,
};
@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let c=clip_from_source_gs*vec4<f32>(input.position,1.0);
    var out: VertexOutput;
    // Explicit backend adaptation: inverse-W depth, reversed GEQUAL, near W=1.
    // Original fixed-point raster/depth quantization is not reproduced here.
    out.position=vec4<f32>((c.x-2048.0*c.w)/256.0,-(c.y-2048.0*c.w)/256.0,1.0,c.w);
    out.uv=input.uv;
    out.color_gs=input.color_gs;
    out.raw_y=c.y/c.w-1792.0;
    return out;
}
fn display_byte_to_linear(value: vec3<f32>) -> vec3<f32> {
    return select(pow((value+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),value/12.92,value<=vec3<f32>(0.04045));
}
@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    // Source-proven opening dialogue black bars in raw512 vertical coordinates.
    if input.raw_y<144.0 || input.raw_y>=368.0 { return vec4<f32>(0.0,0.0,0.0,1.0); }
    // The original sampler is fixed at mip0. An explicit level also avoids
    // implicit derivatives inside this nonuniform black-bar branch.
    let tex=textureSampleLevel(source_texture,source_sampler,input.uv,0.0);
    // Narrow GS byte arithmetic from the reference shader: texture channels,
    // MODULATE and fog each quantize before the following operation.
    let sampled_bytes=trunc(tex.rgb*255.0+vec3<f32>(0.05));
    let modulated_bytes=clamp(trunc(sampled_bytes*input.color_gs.rgb/128.0),vec3<f32>(0.0),vec3<f32>(255.0));
    // Per-object VIF83 is [0,0,0,255], overriding the global distance fog.
    let fogged_bytes=trunc(mix(vec3<f32>(255.0),modulated_bytes,vec3<f32>(255.0/256.0)));
    let byte_color=fogged_bytes/255.0;
    // Bevy's output is sRGB: inverse transfer here keeps byte-space math intact.
    var alpha=1.0;
    if params.x>0.5 { alpha=clamp(tex.a*input.color_gs.a/128.0,0.0,1.0); }
    return vec4<f32>(display_byte_to_linear(byte_color),alpha);
}
