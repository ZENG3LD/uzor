// Depth-only vertex shader for the Wave 7 shadow pass.
//
// Each material's shadow-caster pipeline shares the same Frame
// uniform layout as the main pass except `view_proj` is replaced
// with `light_view_proj` for THIS pass.
//
// Vertex inputs vary per material — we use the SAME layouts as the
// main pipelines (VertexLit / VertexPbr) so the same VBs apply; we
// just ignore everything except pos@0 and the instance model
// columns. Per-mesh-type shadow pipeline lets us keep one shader
// while letting wgpu validate each pipeline against the right
// VB layout.

struct Frame {
    view_proj:       mat4x4<f32>,
    eye:             vec4<f32>,
    light_view_proj: mat4x4<f32>,  // unused in shadow pass (we use view_proj)
    shadow_params:   vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;

// Lit-vertex inputs (matches VertexLit layout, locations 0..2)
struct VsInLit {
    @location(0) pos:    vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color:  vec4<f32>,
    @location(3) m0:     vec4<f32>,
    @location(4) m1:     vec4<f32>,
    @location(5) m2:     vec4<f32>,
    @location(6) m3:     vec4<f32>,
    @location(7) tint:     vec4<f32>,
    @location(8) material: vec4<f32>,
};

@vertex
fn vs_lit(in: VsInLit) -> @builtin(position) vec4<f32> {
    let m = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    return frame.view_proj * (m * vec4<f32>(in.pos, 1.0));
}

// PBR-vertex inputs (matches VertexPbr layout, locations 0..3)
struct VsInPbr {
    @location(0) pos:    vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv:     vec2<f32>,
    @location(4) m0:     vec4<f32>,
    @location(5) m1:     vec4<f32>,
    @location(6) m2:     vec4<f32>,
    @location(7) m3:     vec4<f32>,
    @location(8) tint:     vec4<f32>,
    @location(9) pbr_params: vec4<f32>,
};

@vertex
fn vs_pbr(in: VsInPbr) -> @builtin(position) vec4<f32> {
    let m = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    return frame.view_proj * (m * vec4<f32>(in.pos, 1.0));
}

// Fragment is empty — wgpu requires either no fragment OR an empty
// one for depth-only passes. Choose empty so target list can be [].
