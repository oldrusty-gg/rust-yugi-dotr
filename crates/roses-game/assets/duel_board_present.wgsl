// Presents the Rgba8Unorm board canvas (byte values) on an sRGB target, with
// the duel HUD layer (premultiplied RGBA bytes over the same 512x512 GS
// frame, transparent without duel play) composited first.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var canvas_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var canvas_sampler: sampler;
// x: 1 = final 5-bit pass (restores 248 where additive blending saturated).
// y: the board scene's brightness (0..1): the practice scripts' fade bytes
//    darken the 3D scene but not the HUD layer (tutorial.rs).
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var hud_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var hud_sampler: sampler;
// The cursor unit's canvas (cleared to alpha 0): composited over the scene
// where its alpha bit is set (the original's 0x2000 composite), under the HUD.
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var units_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var units_sampler: sampler;
// The cursor tint added to the unit pixels (RGB bytes, w = 1 when on) and the
// board's DIMX rows for that 16-bit write (params.z = 1).
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> unit_tint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var<uniform> dither_rows: mat4x4<f32>;
// The battle scene's card-versus canvas (cleared to black): params.w = 1
// shows it alone, without the board, the unit canvas or the HUD.
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var versus_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var versus_sampler: sampler;
// Motion blur (the battle scene's unit camera): the previous update's output
// bytes; blur.x = alpha (128 = 1.0), blur.y = 1 writes raw bytes (the
// history target) instead of the display transfer.
@group(#{MATERIAL_BIND_GROUP}) @binding(11) var history_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(12) var history_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(13) var<uniform> blur: vec4<f32>;
// Effects drawn after the HUD (C600): additive bytes over black.
@group(#{MATERIAL_BIND_GROUP}) @binding(14) var top_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(15) var top_sampler: sampler;
// Cards over the HUD (c601's copy): premultiplied RGBA, before `top`.
@group(#{MATERIAL_BIND_GROUP}) @binding(16) var cover_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(17) var cover_sampler: sampler;

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
    let unit = textureSampleLevel(units_texture, units_sampler, input.uv, 0.0);
    if unit.a >= 0.5 {
        bytes = round(unit.rgb * 255.0);
        if unit_tint.w > 0.5 {
            if params.z > 0.5 {
                let p = vec2<u32>(clamp(floor(input.uv * 512.0), vec2<f32>(0.0), vec2<f32>(511.0)));
                let d = dither_rows[p.y & 3u][p.x & 3u];
                bytes = floor(clamp(bytes + unit_tint.rgb + vec3<f32>(d), vec3<f32>(0.0), vec3<f32>(255.0)) / 8.0) * 8.0;
            } else {
                bytes = min(bytes + unit_tint.rgb, vec3<f32>(255.0));
            }
        }
    }
    bytes = floor(bytes * params.y);
    // The battle scene's lights (blur.z = 1 - light / 128) darken the board.
    bytes = floor(bytes * (1.0 - blur.z));
    if params.w > 0.5 {
        bytes = round(textureSampleLevel(versus_texture, versus_sampler, input.uv, 0.0).rgb * 255.0);
    } else {
        // HUD over the scene: out = hud + (1 - coverage) * scene, in bytes.
        let hud = textureSampleLevel(hud_texture, hud_sampler, input.uv, 0.0);
        bytes = min(round(hud.rgb * 255.0 + (1.0 - hud.a) * bytes), vec3<f32>(255.0));
        let cover = textureSampleLevel(cover_texture, cover_sampler, input.uv, 0.0);
        bytes = min(round(cover.rgb * 255.0 + (1.0 - cover.a) * bytes), vec3<f32>(255.0));
        let top = round(textureSampleLevel(top_texture, top_sampler, input.uv, 0.0).rgb * 255.0);
        bytes = min(bytes + top, vec3<f32>(255.0));
    }
    if blur.x > 0.0 {
        // GS ALPHA (Cs - Cd) * As + Cd with the previous frame as Cs.
        let previous = round(textureSampleLevel(history_texture, history_sampler, input.uv, 0.0).rgb * 255.0);
        bytes = bytes + trunc((previous - bytes) * blur.x / 128.0);
    }
    if params.x > 0.5 {
        bytes = floor(bytes / 8.0) * 8.0;
    }
    if blur.y > 0.5 {
        return vec4<f32>(bytes / 255.0, 1.0);
    }
    // The present camera's target is sRGB: the inverse transfer keeps bytes.
    return vec4<f32>(display_byte_to_linear(bytes / 255.0), 1.0);
}
