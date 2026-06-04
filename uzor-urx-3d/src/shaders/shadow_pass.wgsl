// Depth-only vertex shader for the Wave 7 shadow pass + Wave 14
// textured caster.
//
// Each material's shadow-caster pipeline shares the same Frame
// uniform layout as the main pass except `view_proj` is replaced
// with `light_view_proj` for THIS pass.
//
// Vertex inputs match the main pipelines so the same VBs apply;
// only `pos@0` and the instance model columns are needed here.

struct Frame {
    view_proj:       mat4x4<f32>,
    eye:             vec4<f32>,
    light_view_proj: mat4x4<f32>,  // unused — view_proj already swapped on CPU
    shadow_params:   vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;

// Lit/Tex per-instance layouts — same struct (InstanceLitRaw), shadow
// pass ignores everything past model columns. Wave 11 added normal
// matrix at locations 9..11 — those must still be declared so wgpu
// vertex state validation matches the VB layout, but they go unused.

// VertexLit per-vertex inputs (locations 0..2) + InstanceLit (3..11).
struct VsInLit {
    @location(0) pos:      vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) color:    vec4<f32>,
    @location(3) m0:       vec4<f32>,
    @location(4) m1:       vec4<f32>,
    @location(5) m2:       vec4<f32>,
    @location(6) m3:       vec4<f32>,
    @location(7) tint:     vec4<f32>,
    @location(8) material: vec4<f32>,
    @location(9)  nmat_c0: vec4<f32>,
    @location(10) nmat_c1: vec4<f32>,
    @location(11) nmat_c2: vec4<f32>,
};

@vertex
fn vs_lit(in: VsInLit) -> @builtin(position) vec4<f32> {
    let m = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    return frame.view_proj * (m * vec4<f32>(in.pos, 1.0));
}

// VertexUv per-vertex inputs (locations 0..2) + InstanceLit (3..11).
struct VsInTex {
    @location(0) pos:      vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
    @location(3) m0:       vec4<f32>,
    @location(4) m1:       vec4<f32>,
    @location(5) m2:       vec4<f32>,
    @location(6) m3:       vec4<f32>,
    @location(7) tint:     vec4<f32>,
    @location(8) material: vec4<f32>,
    @location(9)  nmat_c0: vec4<f32>,
    @location(10) nmat_c1: vec4<f32>,
    @location(11) nmat_c2: vec4<f32>,
};

@vertex
fn vs_tex(in: VsInTex) -> @builtin(position) vec4<f32> {
    let m = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    return frame.view_proj * (m * vec4<f32>(in.pos, 1.0));
}

// VertexPbr per-vertex inputs (locations 0..3) + InstancePbr (4..12).
struct VsInPbr {
    @location(0) pos:        vec3<f32>,
    @location(1) normal:     vec3<f32>,
    @location(2) tangent:    vec4<f32>,
    @location(3) uv:         vec2<f32>,
    @location(4) m0:         vec4<f32>,
    @location(5) m1:         vec4<f32>,
    @location(6) m2:         vec4<f32>,
    @location(7) m3:         vec4<f32>,
    @location(8) tint:       vec4<f32>,
    @location(9) pbr_params: vec4<f32>,
    @location(10) nmat_c0:   vec4<f32>,
    @location(11) nmat_c1:   vec4<f32>,
    @location(12) nmat_c2:   vec4<f32>,
};

@vertex
fn vs_pbr(in: VsInPbr) -> @builtin(position) vec4<f32> {
    let m = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    return frame.view_proj * (m * vec4<f32>(in.pos, 1.0));
}
