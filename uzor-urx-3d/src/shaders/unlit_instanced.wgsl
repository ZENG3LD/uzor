// Unlit + instanced vertex/fragment for URX 3D Wave 3.
//
// Per-frame: ViewProj (binding 0).
// Per-instance: model matrix + tint, packed in a vertex buffer at
// step_mode = Instance. One drawcall paints N instances of the same
// mesh — collapses the Wave 1+2 per-node-loop into a single
// draw_indexed_instanced.

struct Frame {
    view_proj: mat4x4<f32>,
    eye:       vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;

struct VsIn {
    @location(0) pos:   vec3<f32>,
    @location(1) color: vec4<f32>,
    // Instance row-vectors of the model matrix (4 vec4s + tint at @5..@9)
    @location(2) model_c0: vec4<f32>,
    @location(3) model_c1: vec4<f32>,
    @location(4) model_c2: vec4<f32>,
    @location(5) model_c3: vec4<f32>,
    @location(6) tint:     vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(in.model_c0, in.model_c1, in.model_c2, in.model_c3);
    let world = model * vec4<f32>(in.pos, 1.0);
    out.clip = frame.view_proj * world;
    out.color = in.color * in.tint;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
