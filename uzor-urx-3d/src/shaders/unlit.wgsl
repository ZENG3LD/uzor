// Unlit vertex/fragment for URX 3D Wave 1.
//
// Per-frame: ViewProj (binding 0).
// Per-node:  Model + tint (push-constant-style — uploaded as a separate
//            small uniform buffer per draw call).
//
// Output: vertex.color × node.tint, no lighting (Wave 3 adds Phong).

struct Frame {
    view_proj:       mat4x4<f32>,
    eye:             vec4<f32>,
    light_view_proj: mat4x4<f32>,
    shadow_params:   vec4<f32>,
};

struct NodeData {
    model: mat4x4<f32>,
    tint:  vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> node:  NodeData;

struct VsIn {
    @location(0) pos:   vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world = node.model * vec4<f32>(in.pos, 1.0);
    out.clip = frame.view_proj * world;
    out.color = in.color * node.tint;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
