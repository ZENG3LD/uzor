// Wave 12 — HDR → swapchain composite with ACES tonemap + gamma 2.2.
// Reads the Rgba16Float HDR buffer, adds the bloom pyramid sample,
// tonemaps with the cheap Krzysztof Narkowicz fit of ACES, then
// gamma-encodes to sRGB-display.

@group(0) @binding(0) var t_hdr:    texture_2d<f32>;
@group(0) @binding(1) var s_hdr:    sampler;
@group(0) @binding(2) var t_bloom:  texture_2d<f32>;
@group(0) @binding(3) var s_bloom:  sampler;

struct Params {
    bloom_strength: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(0) @binding(4) var<uniform> params: Params;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    // Fullscreen triangle covering NDC (-1..3, -1..3) — common trick
    // (3 vertices instead of 6, avoids the diagonal seam).
    var out: VsOut;
    let xy = vec2<f32>(f32((idx << 1u) & 2u), f32(idx & 2u));
    out.clip = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    // UV: 0..1 corresponds to xy.x in [0,1], y FLIPPED so origin matches
    // wgpu's top-left framebuffer convention.
    out.uv = vec2<f32>(xy.x, 1.0 - xy.y);
    return out;
}

// ACES Filmic tonemap — Narkowicz fit (https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/)
fn aces_filmic(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let hdr = textureSample(t_hdr, s_hdr, in.uv).rgb;
    let bloom = textureSample(t_bloom, s_bloom, in.uv).rgb;
    let lit = hdr + bloom * params.bloom_strength;
    let tonemapped = aces_filmic(lit);
    let gamma = pow(tonemapped, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(gamma, 1.0);
}
