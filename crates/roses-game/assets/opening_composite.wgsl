@group(#{MATERIAL_BIND_GROUP}) @binding(0) var world_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var world_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var overlay_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var overlay_sampler: sampler;
// Full-screen scene overlay quad: byte-space RGB/255 and GS alpha/128.
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> cover: vec4<f32>;
// Additive scene flash, byte-space RGB/255 already scaled by As/128.
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> flash: vec4<f32>;
// c401's light beam: additive RGB bytes for raw512 rows 144..368 (512 x 224).
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var beam_texture: texture_2d<f32>;
struct VertexInput { @location(0) position: vec3<f32>, @location(1) uv: vec2<f32> };
struct VertexOutput { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vertex(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position=vec4<f32>(input.position.xy,0.5,1.0);
    out.uv=input.uv;
    return out;
}
fn from_srgb(value: vec3<f32>) -> vec3<f32> {
    return select(pow((value+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),value/12.92,value<=vec3<f32>(0.04045));
}
fn to_srgb(value: vec3<f32>) -> vec3<f32> {
    return select(1.055*pow(max(value,vec3<f32>(0.0)),vec3<f32>(1.0/2.4))-vec3<f32>(0.055),value*12.92,value<=vec3<f32>(0.0031308));
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    // The world target is sRGB, so sampling decodes it. Restore byte-space RGB
    // before mixing with the client's already-premultiplied, unorm overlay.
    let scene=to_srgb(textureSampleLevel(world_texture,world_sampler,input.uv,0.0).rgb);
    // GS (Cs-Cd)*As+Cd in normalized byte space; 5-bit framebuffer rounding is not modelled here.
    let raw_y=input.uv.y*512.0;
    var lit=scene;
    // The beam draws with the 3D scene (Cd + Cs*As/128, saturating), under
    // the overlay quad and the flash.
    if raw_y>=144.0 && raw_y<368.0 {
        let texel=vec2<i32>(clamp(i32(input.uv.x*512.0),0,511),clamp(i32(raw_y-144.0),0,223));
        lit=min(scene+textureLoad(beam_texture,texel,0).rgb,vec3<f32>(1.0));
    }
    var world=lit+(cover.rgb-lit)*cover.a;
    // GS Cd + Cs*As/128 over the 3D band only; the letterbox bars stay black.
    if raw_y>=144.0 && raw_y<368.0 { world=min(world+flash.rgb,vec3<f32>(1.0)); }
    let overlay=textureSampleLevel(overlay_texture,overlay_sampler,input.uv,0.0);
    let bytes=clamp(overlay.rgb+world*(1.0-overlay.a),vec3<f32>(0.0),vec3<f32>(1.0));
    return vec4<f32>(from_srgb(bytes),1.0);
}
