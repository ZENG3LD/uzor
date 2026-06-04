// Wave 9b ext — region blit shader.
// Reads one region's intermediate texture and writes it to the
// destination framebuffer at the region's bounds. The bounds → NDC
// transform is supplied via push-constant-like uniform.

struct Params {
    /// xy = min in NDC (-1..1), zw = max in NDC (-1..1)
    rect: vec4<f32>,
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var s_src: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    // 4-vertex triangle strip
    var corner: vec2<f32> = vec2<f32>(0.0);
    var uv: vec2<f32> = vec2<f32>(0.0);
    switch (idx) {
        case 0u: { corner = vec2<f32>(params.rect.x, params.rect.y); uv = vec2<f32>(0.0, 1.0); }
        case 1u: { corner = vec2<f32>(params.rect.z, params.rect.y); uv = vec2<f32>(1.0, 1.0); }
        case 2u: { corner = vec2<f32>(params.rect.x, params.rect.w); uv = vec2<f32>(0.0, 0.0); }
        case 3u: { corner = vec2<f32>(params.rect.z, params.rect.w); uv = vec2<f32>(1.0, 0.0); }
        default: {}
    }
    var out: VsOut;
    out.clip = vec4<f32>(corner, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t_src, s_src, in.uv);
}
