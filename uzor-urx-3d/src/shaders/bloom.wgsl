// Wave 12 — bloom pyramid: down13 + up tent. Each pass = fullscreen
// triangle reading one mip, writing the next. The HDR composite
// adds the FINAL upsampled mip to the original HDR buffer.
//
// down13 — 13-tap downsample (CoD AW blur), bright-pass on level 0
// only via the threshold uniform.
// up — 3×3 tent upsample blended additively.

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

struct Params {
    threshold: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(0) @binding(2) var<uniform> params: Params;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var out: VsOut;
    let xy = vec2<f32>(f32((idx << 1u) & 2u), f32(idx & 2u));
    out.clip = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(xy.x, 1.0 - xy.y);
    return out;
}

fn brightpass(c: vec3<f32>, threshold: f32) -> vec3<f32> {
    let l = max(max(c.r, c.g), c.b);
    let scale = max(0.0, l - threshold) / max(l, 1e-4);
    return c * scale;
}

@fragment
fn fs_down13(in: VsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(t_src, 0));
    let tx = 1.0 / dims.x;
    let ty = 1.0 / dims.y;

    // 13-tap CoD downsample (5 corner-tap + 4 inner-tap + centre).
    let a = textureSample(t_src, s_src, in.uv + vec2<f32>(-2.0 * tx,  2.0 * ty)).rgb;
    let b = textureSample(t_src, s_src, in.uv + vec2<f32>( 0.0,       2.0 * ty)).rgb;
    let c = textureSample(t_src, s_src, in.uv + vec2<f32>( 2.0 * tx,  2.0 * ty)).rgb;
    let d = textureSample(t_src, s_src, in.uv + vec2<f32>(-2.0 * tx,  0.0)).rgb;
    let e = textureSample(t_src, s_src, in.uv).rgb;
    let f = textureSample(t_src, s_src, in.uv + vec2<f32>( 2.0 * tx,  0.0)).rgb;
    let g = textureSample(t_src, s_src, in.uv + vec2<f32>(-2.0 * tx, -2.0 * ty)).rgb;
    let h = textureSample(t_src, s_src, in.uv + vec2<f32>( 0.0,      -2.0 * ty)).rgb;
    let i = textureSample(t_src, s_src, in.uv + vec2<f32>( 2.0 * tx, -2.0 * ty)).rgb;

    let j = textureSample(t_src, s_src, in.uv + vec2<f32>(-tx,  ty)).rgb;
    let k = textureSample(t_src, s_src, in.uv + vec2<f32>( tx,  ty)).rgb;
    let l = textureSample(t_src, s_src, in.uv + vec2<f32>(-tx, -ty)).rgb;
    let m = textureSample(t_src, s_src, in.uv + vec2<f32>( tx, -ty)).rgb;

    // Weights from the original paper. Total = 1.0.
    var color = e * 0.125
              + (a + c + g + i) * 0.03125
              + (b + d + f + h) * 0.0625
              + (j + k + l + m) * 0.125;

    // Threshold > 0 means this is the BRIGHT-PASS (down 0→1); above
    // that level threshold should be 0 and bright pass acts as identity.
    if (params.threshold > 0.0) {
        color = brightpass(color, params.threshold);
    }
    return vec4<f32>(color, 1.0);
}

@fragment
fn fs_up_tent(in: VsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(t_src, 0));
    let tx = 1.0 / dims.x;
    let ty = 1.0 / dims.y;

    // 3×3 tent filter.
    let a = textureSample(t_src, s_src, in.uv + vec2<f32>(-tx,  ty)).rgb;
    let b = textureSample(t_src, s_src, in.uv + vec2<f32>(0.0,  ty)).rgb;
    let c = textureSample(t_src, s_src, in.uv + vec2<f32>( tx,  ty)).rgb;
    let d = textureSample(t_src, s_src, in.uv + vec2<f32>(-tx, 0.0)).rgb;
    let e = textureSample(t_src, s_src, in.uv).rgb;
    let f = textureSample(t_src, s_src, in.uv + vec2<f32>( tx, 0.0)).rgb;
    let g = textureSample(t_src, s_src, in.uv + vec2<f32>(-tx, -ty)).rgb;
    let h = textureSample(t_src, s_src, in.uv + vec2<f32>(0.0, -ty)).rgb;
    let i = textureSample(t_src, s_src, in.uv + vec2<f32>( tx, -ty)).rgb;

    let color = (e * 4.0 + (b + d + f + h) * 2.0 + (a + c + g + i)) * (1.0 / 16.0);
    return vec4<f32>(color, 1.0);
}
