// Instanced textured-Phong for URX 3D Wave 5.
//
// @group(0): frame + lights (same layout as phong_instanced)
// @group(1): texture + sampler
//
// Vertex inputs (per-vertex VB layout 0):
//   @0 pos    : vec3<f32>
//   @1 normal : vec3<f32>
//   @2 uv     : vec2<f32>
//
// Instance inputs (per-instance VB layout 1):
//   @3 model_c0..@6 model_c3 : vec4<f32>
//   @7 tint                  : vec4<f32>
//   @8 material              : vec4<f32>  ambient_k, diffuse_k, spec_k, shininess

const MAX_LIGHTS: u32 = 8u;

struct Frame {
    view_proj: mat4x4<f32>,
    eye:       vec4<f32>,
};

struct LightSlot {
    kind:      u32,
    _pad0_a:   u32,
    _pad0_b:   u32,
    _pad0_c:   u32,
    vec:       vec3<f32>,
    _pad1:     f32,
    color:     vec3<f32>,
    intensity: f32,
    range:     f32,
    _pad2:     vec3<f32>,
};

struct LightArrayU {
    count:    u32,
    _pad_a:   u32,
    _pad_b:   u32,
    _pad_c:   u32,
    ambient:  vec3<f32>,
    _pad_amb: f32,
    lights:   array<LightSlot, MAX_LIGHTS>,
};

@group(0) @binding(0) var<uniform> frame:  Frame;
@group(0) @binding(1) var<uniform> lights: LightArrayU;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

struct VsIn {
    @location(0) pos:    vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv:     vec2<f32>,
    @location(3) model_c0: vec4<f32>,
    @location(4) model_c1: vec4<f32>,
    @location(5) model_c2: vec4<f32>,
    @location(6) model_c3: vec4<f32>,
    @location(7) tint:     vec4<f32>,
    @location(8) material: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip:   vec4<f32>,
    @location(0) world_pos:    vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv:           vec2<f32>,
    @location(3) tint:         vec4<f32>,
    @location(4) material:     vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(in.model_c0, in.model_c1, in.model_c2, in.model_c3);
    let world = model * vec4<f32>(in.pos, 1.0);
    out.world_pos = world.xyz;
    let m3 = mat3x3<f32>(model[0].xyz, model[1].xyz, model[2].xyz);
    out.world_normal = normalize(m3 * in.normal);
    out.clip = frame.view_proj * world;
    out.uv = in.uv;
    out.tint = in.tint;
    out.material = in.material;
    return out;
}

fn light_contribution(
    l: LightSlot,
    world_pos: vec3<f32>,
    n: vec3<f32>,
    view_dir: vec3<f32>,
    diffuse_k: f32,
    specular_k: f32,
    shininess: f32,
) -> vec3<f32> {
    var to_l: vec3<f32>;
    var atten: f32;
    if (l.kind == 0u) {
        to_l = normalize(-l.vec);
        atten = 1.0;
    } else if (l.kind == 1u) {
        let d_vec = l.vec - world_pos;
        let dist = length(d_vec);
        to_l = d_vec / max(dist, 0.0001);
        let r = max(l.range, 0.0001);
        let x = clamp(dist / r, 0.0, 1.0);
        atten = (1.0 - x) * (1.0 - x);
    } else {
        return vec3<f32>(0.0);
    }
    let ndotl = max(dot(n, to_l), 0.0);
    let diff = ndotl * diffuse_k;
    let h = normalize(to_l + view_dir);
    let ndoth = max(dot(n, h), 0.0);
    let spec = pow(ndoth, max(shininess, 1.0)) * specular_k * ndotl;
    return l.color * l.intensity * atten * (diff + spec);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(t_diffuse, s_diffuse, in.uv);
    let base = texel * in.tint;

    let n = normalize(in.world_normal);
    let view_dir = normalize(frame.eye.xyz - in.world_pos);

    let ambient_k  = in.material.x;
    let diffuse_k  = in.material.y;
    let specular_k = in.material.z;
    let shininess  = in.material.w;

    var rgb = lights.ambient * ambient_k * base.rgb;

    var i = 0u;
    loop {
        if (i >= lights.count) { break; }
        if (i >= MAX_LIGHTS) { break; }
        let l = lights.lights[i];
        rgb = rgb + light_contribution(l, in.world_pos, n, view_dir,
                                       diffuse_k, specular_k, shininess) * base.rgb;
        i = i + 1u;
    }

    return vec4<f32>(rgb, base.a);
}
