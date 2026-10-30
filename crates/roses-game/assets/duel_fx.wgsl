// Battle effects with per-vertex GS colour (c621, the hand-ported C modules
// and the attack effects' captured primitives): positions in GS window
// coordinates, colour bytes with alpha 128 = 1.0 (up to 255), optionally
// textured (PNG alpha 255 = GS 128) with the GS texture function; written
// additively as Cs * A / 128 + Cd (ALPHA 0x48), subtractively, normally
// (0x44) or replacing, without depth.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ndc_from_source: mat4x4<f32>;
// x: 1 = textured, y: 1 = 16-bit C_16S write with DTHE, z: 1 = ALPHA 0x44,
// w: 1 = position z is the depth (GS Z24: (Z + 2048.25) / 16779264 = 1 / w).
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> dither_rows: mat4x4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var source_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var source_sampler: sampler;
// x: TFX (0 modulate, 1 decal, 2 / 3 highlight), y: TCC, z: the blend's
// alpha (0 As, 1 FIX = w, 2 unblended: 128).
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> gs: vec4<f32>;
// The attack effects' GS sampling (GS_TEX): CLAMP WMS, WMT (0 repeat, 1
// clamp, 2 region clamp, 3 region repeat), TEX1 MMAG / MMIN linear; the
// region MINU, MAXU, MINV, MAXV; 2^TW, 2^TH, LOD K, L; the image's size in
// GS texels (UVs are normalised to it).
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> wrap: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> region: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var<uniform> lod: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var<uniform> size: vec4<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
#ifdef GS_TEX
    // (the Q dividing uv: Q for ST, 1 for UV; the RGBAQ Q for the LOD)
    @location(3) q: vec2<f32>,
#endif
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
#ifdef GS_TEX
    @location(2) q: vec2<f32>,
#endif
};

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let c = ndc_from_source * vec4<f32>(input.position, 1.0);
    out.position = vec4<f32>(c.x, c.y, select(1.0, input.position.z, params.w > 0.5) * c.w, c.w);
    out.uv = input.uv;
    out.color = input.color;
#ifdef GS_TEX
    out.q = input.q;
#endif
    return out;
}

#ifdef GS_TEX
// One texel address on one axis by the CLAMP mode (the GS's wrap).
fn wrap_axis(i: i32, mode: u32, n: i32, lo: i32, hi: i32) -> i32 {
    if mode == 0u {
        return i & (n - 1);
    } else if mode == 1u {
        return clamp(i, 0, n - 1);
    } else if mode == 2u {
        return clamp(i, lo, hi);
    }
    return (i & lo) | hi;
}

fn texel(i: vec2<i32>) -> vec4<f32> {
    let u = wrap_axis(i.x, u32(wrap.x + 0.5), i32(lod.x + 0.5), i32(region.x + 0.5), i32(region.y + 0.5));
    let v = wrap_axis(i.y, u32(wrap.y + 0.5), i32(lod.y + 0.5), i32(region.z + 0.5), i32(region.w + 0.5));
    let dims = vec2<f32>(textureDimensions(source_texture));
    // The GS texel's centre in the image (a canvas may hold it scaled).
    let p = vec2<i32>(floor((vec2<f32>(f32(u), f32(v)) + 0.5) * dims / max(size.xy, vec2<f32>(1.0))));
    return textureLoad(source_texture, clamp(p, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1)), 0);
}

// One pixel of the frame image as the C_16S word the GS holds (5 bits per
// channel, the A bit from alpha >= 0.5 [H: the port's frame alpha]).
fn frame_half(x: i32) -> u32 {
    let dims = vec2<f32>(textureDimensions(source_texture));
    let p = vec2<i32>(floor((vec2<f32>(f32(x), 0.0) + 0.5) * dims / max(size.xy, vec2<f32>(1.0))));
    let c = textureLoad(source_texture, clamp(p, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1)), 0);
    let b = vec4<u32>(floor(clamp(c, vec4<f32>(0.0), vec4<f32>(1.0)) * 255.0 + 0.5));
    return (b.x >> 3u) | ((b.y >> 3u) << 5u) | ((b.z >> 3u) << 10u) | (select(0u, 1u, b.w >= 128u) << 15u);
}

// size.z = 1: TEX0 PSMCT32 1 x 1 at the frame buffer's TBP: the 32-bit word
// holding pixels (0, 0) (low half) and (8, 0) (high half) [F PSMCT16S
// column layout; emu card 133 c200: magenta strips].
fn frame_word() -> vec4<f32> {
    let lo = frame_half(0);
    let hi = frame_half(8);
    let t = vec4<u32>(lo & 0xFFu, lo >> 8u, hi & 0xFFu, hi >> 8u);
    // The caller reads texel alpha * 128: the A byte as is (128 = 1.0).
    return vec4<f32>(vec3<f32>(t.xyz) / 255.0, f32(t.w) / 128.0);
}

// S / Q and T / Q per pixel, the LOD from Q (LCM 0: log2(1 / Q) << L + K),
// MMAG below LOD 0 else MMIN, bilinear about texel centres.
fn gs_sample(uv: vec2<f32>, q: vec2<f32>) -> vec4<f32> {
    if size.z > 0.5 {
        return frame_word();
    }
    let g = uv / select(q.x, 1.0, q.x == 0.0) * size.xy;
    let level = log2(1.0 / max(abs(q.y), 1e-20)) * exp2(lod.w) + lod.z;
    let linear = select(wrap.w, wrap.z, level <= 0.0) > 0.5;
    if !linear {
        return texel(vec2<i32>(floor(g)));
    }
    let f = g - vec2<f32>(0.5);
    let i = vec2<i32>(floor(f));
    let w = fract(f);
    let top = mix(texel(i), texel(i + vec2<i32>(1, 0)), w.x);
    let bottom = mix(texel(i + vec2<i32>(0, 1)), texel(i + vec2<i32>(1, 1)), w.x);
    return mix(top, bottom, w.y);
}
#endif

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let p = vec2<u32>(input.position.xy);
    let d = dither_rows[p.y & 3u][p.x & 3u];
    // Gouraud colour, interpolated in GS steps (truncated).
    var c = trunc(input.color.rgb);
    var a = trunc(input.color.a);
    if params.x > 0.5 {
#ifdef GS_TEX
        let tex = gs_sample(input.uv, input.q);
#else
        let tex = textureSampleLevel(source_texture, source_sampler, input.uv, 0.0);
#endif
        let t = trunc(tex.rgb * 255.0 + vec3<f32>(0.05));
        let ta = trunc(tex.a * 128.0 + 0.05);
        let tfx = u32(gs.x + 0.5);
        if tfx == 1u {
            c = t;
            a = select(a, ta, gs.y > 0.5);
        } else if tfx >= 2u {
            c = min(trunc(t * c / 128.0) + a, vec3<f32>(255.0));
            a = select(a, min(ta + a, 255.0), gs.y > 0.5);
        } else {
            c = min(trunc(t * c / 128.0), vec3<f32>(255.0));
            a = select(a, min(trunc(ta * a / 128.0), 255.0), gs.y > 0.5);
        }
    }
    // Alpha test (GREATER 0) on the source alpha, then the blend's alpha.
    if gs.z < 1.5 && a <= 0.0 {
        discard;
    }
    if gs.z > 1.5 {
        a = 128.0;
    } else if gs.z > 0.5 {
        a = gs.w;
    }
    if params.z > 0.5 {
        // ALPHA 0x44: (Cs - Cd) As + Cd, 0x84: (Cs - Cd) As; the blend
        // state applies As.
        return vec4<f32>(min(c, vec3<f32>(255.0)) / 255.0, min(a / 128.0, 1.0));
    }
    let o = min(trunc(c * a / 128.0), vec3<f32>(255.0));
    let dithered = floor(clamp(o + vec3<f32>(d), vec3<f32>(0.0), vec3<f32>(255.0)) / 8.0) * 8.0;
    let rgb = select(o, dithered, params.y > 0.5);
    return vec4<f32>(rgb / 255.0, 1.0);
}
