@group(#{MATERIAL_BIND_GROUP}) @binding(0) var atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<storage, read> vertices: array<vec4<f32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<storage, read> bins: array<u32>;
struct Settings {
    dither0: vec4<i32>, dither1: vec4<i32>, dither2: vec4<i32>, dither3: vec4<i32>,
    fade0: vec4<i32>, fade1: vec4<i32>, fade2: vec4<i32>, fade3: vec4<i32>,
    fade_alpha: vec4<i32>,
};
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> settings: Settings;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var world: texture_2d<f32>;
struct VertexInput { @location(0) position: vec3<f32> };
struct VertexOutput { @builtin(position) position: vec4<f32> };
@vertex fn vertex(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position=vec4<f32>(input.position.xy,0.5,1.0);
    return out;
}
fn edge(a: vec2<f32>, b: vec2<f32>, p: vec2<f32>) -> f32 {
    return (b.x-a.x)*(p.y-a.y)-(b.y-a.y)*(p.x-a.x);
}
fn top_left(a: vec2<f32>, b: vec2<f32>) -> bool {
    return b.y<a.y || (b.y==a.y && b.x>a.x);
}
// XY and UV are affine in this FST triangle strip. Return -1 for no coverage.
fn triangle(p: vec2<f32>, aa: vec4<f32>, bb: vec4<f32>, cc: vec4<f32>) -> vec2<f32> {
    let a=aa;
    var b=bb;
    var c=cc;
    var area=edge(a.xy,b.xy,c.xy);
    if area<0.0 { let swap=b; b=c; c=swap; area=-area; }
    if area==0.0 { return vec2<f32>(-1.0); }
    let w=vec3<f32>(edge(b.xy,c.xy,p),edge(c.xy,a.xy,p),edge(a.xy,b.xy,p));
    if any(w<vec3<f32>(0.0)) || (w.x==0.0 && !top_left(b.xy,c.xy))
        || (w.y==0.0 && !top_left(c.xy,a.xy)) || (w.z==0.0 && !top_left(a.xy,b.xy)) {
        return vec2<f32>(-1.0);
    }
    let q=w/area;
    return a.zw*q.x+b.zw*q.y+c.zw*q.z;
}
fn texel(p: vec2<i32>) -> vec4<f32> {
    let wrapped=((p%vec2<i32>(128,64))+vec2<i32>(128,64))%vec2<i32>(128,64);
    return round(textureLoad(atlas,wrapped,0)*255.0);
}
fn sampled_bytes(uv: vec2<f32>) -> vec4<f32> {
    let at=uv-vec2<f32>(0.5);
    let lower=vec2<i32>(floor(at));
    let q=fract(at);
    let a=mix(texel(lower),texel(lower+vec2<i32>(1,0)),q.x);
    let b=mix(texel(lower+vec2<i32>(0,1)),texel(lower+vec2<i32>(1,1)),q.x);
    return trunc(mix(a,b,q.y)+vec4<f32>(0.05));
}
fn to_srgb(value: vec3<f32>) -> vec3<f32> {
    return select(1.055*pow(max(value,vec3<f32>(0.0)),vec3<f32>(1.0/2.4))-vec3<f32>(0.055),value*12.92,value<=vec3<f32>(0.0031308));
}
fn to_linear(value: vec3<f32>) -> vec3<f32> {
    return select(pow((value+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),value/12.92,value<=vec3<f32>(0.04045));
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel=vec2<u32>(input.position.xy);
    if pixel.y<144u || pixel.y>=368u { return vec4<f32>(0.0,0.0,0.0,1.0); }
    let slot=((pixel.y/16u)*32u+pixel.x/16u)*2u;
    let first=bins[slot];
    let count=bins[slot+1u];
    // Opaque navy fade (0,0,32): its blend yields Cs, then its own DIMX and 5 bits.
    let fades=array<vec4<i32>,4>(settings.fade0,settings.fade1,settings.fade2,settings.fade3);
    let fade=f32(fades[pixel.y%4u][pixel.x%4u]);
    // Native Stonehenge world beneath (sRGB target back to bytes, cut to 5 bits;
    // its own original dither is not modelled), then the navy quad
    // (Cs-Cd)*As>>7+Cd with the fade DIMX. At alpha 128 the world is hidden.
    let a=f32(settings.fade_alpha.x);
    var under=vec3<f32>(0.0);
    if a<128.0 {
        let w=textureLoad(world,vec2<i32>(pixel),0).rgb;
        under=floor(round(to_srgb(w)*255.0)/8.0)*8.0;
    }
    let navy=floor((vec3<f32>(0.0,0.0,32.0)-under)*a/128.0+under);
    var dst=floor(clamp(navy+vec3<f32>(fade),vec3<f32>(0.0),vec3<f32>(255.0))/8.0)*8.0;
    let rows=array<vec4<i32>,4>(settings.dither0,settings.dither1,settings.dither2,settings.dither3);
    let dither=f32(rows[pixel.y%4u][pixel.x%4u]);
    // Original ordering and 5-bit quantization after EVERY blend are essential.
    for(var i=0u;i<count;i=i+1u) {
        let base=bins[first+i]*5u;
        let a=vertices[base]; let b=vertices[base+1u];
        let c=vertices[base+2u]; let d=vertices[base+3u];
        var uv=triangle(input.position.xy,a,b,c);
        if uv.x<0.0 { uv=triangle(input.position.xy,c,b,d); }
        if uv.x<0.0 { continue; }
        let tex=sampled_bytes(uv);
        let color=vertices[base+4u];
        let modulated=min(trunc(tex*color/128.0),vec4<f32>(255.0));
        if modulated.a<=0.0 { continue; }
        let blend=floor((modulated.rgb-dst)*(modulated.a/128.0)+dst);
        dst=floor(clamp(blend+vec3<f32>(dither),vec3<f32>(0.0),vec3<f32>(255.0))/8.0)*8.0;
    }
    return vec4<f32>(to_linear(dst/255.0),1.0);
}
