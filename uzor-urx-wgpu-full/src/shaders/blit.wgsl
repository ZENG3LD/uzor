// blit.wgsl — fullscreen triangle blit from rgba8unorm storage texture
// to any render-attachment format.
//
// Vertex stage: generates a big triangle covering clip-space [-1,-1]..[1,1]
// in exactly 3 vertices with no vertex buffer (index-driven from vertex_index).
//
// Fragment stage: samples `src` at the pixel's UV coordinate (derived from
// fragment position + src_size uniform) and writes straight to the render
// attachment.
//
// Bind group layout (group 0):
//   binding 0 — texture_2d<f32>    src       (the rgba8unorm storage tex)
//   binding 1 — sampler             src_smp   (linear, clamp-to-edge)
//   binding 2 — uniform vec4<u32>   src_size  (w, h, 0, 0)

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    // Big-triangle trick: 3 vertices cover the full NDC square.
    //   vi=0 → (-1, -1)
    //   vi=1 → ( 3, -1)
    //   vi=2 → (-1,  3)
    let x = f32((vi & 1u) << 2u) - 1.0;
    let y = f32((vi & 2u) << 1u) - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@group(0) @binding(0) var src:      texture_2d<f32>;
@group(0) @binding(1) var src_smp:  sampler;
@group(0) @binding(2) var<uniform>  src_size: vec4<u32>; // x=w, y=h, z=_, w=_

@fragment
fn fs_main(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(frag_pos.x / f32(src_size.x), frag_pos.y / f32(src_size.y));
    return textureSample(src, src_smp, uv);
}
