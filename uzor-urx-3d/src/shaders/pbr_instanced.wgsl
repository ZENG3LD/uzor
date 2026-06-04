// Instanced PBR (Cook-Torrance) for URX 3D Wave 6.
//
// Microfacet BRDF: GGX (Trowbridge-Reitz) NDF, Smith geometry,
// Schlick Fresnel. F0 = mix(0.04, albedo, metalness) — dielectric vs
// conductor split.
//
// @group(0): frame + lights (same layout as phong/textured)
// @group(1): albedo texture + sampler
// @group(2): normal map texture + sampler (single 1x1 stub when no
//           normal map; sampled but multiplied by has_normal_map flag)
//
// Vertex inputs (per-vertex VB layout 0):
//   @0 pos     : vec3<f32>
//   @1 normal  : vec3<f32>
//   @2 tangent : vec4<f32>  xyz = tangent dir, w = handedness ±1
//   @3 uv      : vec2<f32>
//
// Instance inputs (per-instance VB layout 1):
//   @4 model_c0..@7 model_c3 : vec4<f32>
//   @8 tint                  : vec4<f32>
//   @9 pbr_params            : vec4<f32>  metalness, roughness, ao,
//                                          has_normal_map (0 or 1)

const MAX_LIGHTS: u32 = 8u;
const PI: f32 = 3.14159265358979;

struct Frame {
    view_proj:       mat4x4<f32>,
    eye:             vec4<f32>,
    light_view_proj: mat4x4<f32>,
    shadow_params:   vec4<f32>,
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

@group(1) @binding(0) var t_albedo: texture_2d<f32>;
@group(1) @binding(1) var s_albedo: sampler;

@group(2) @binding(0) var t_normal: texture_2d<f32>;
@group(2) @binding(1) var s_normal: sampler;

@group(3) @binding(0) var t_shadow: texture_depth_2d;
@group(3) @binding(1) var s_shadow: sampler_comparison;

fn sample_shadow(world_pos: vec3<f32>) -> f32 {
    if (frame.shadow_params.x < 0.5) { return 1.0; }
    let clip = frame.light_view_proj * vec4<f32>(world_pos, 1.0);
    let proj = clip.xyz / clip.w;
    let uv = vec2<f32>(proj.x * 0.5 + 0.5, -proj.y * 0.5 + 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || proj.z > 1.0) { return 1.0; }
    let bias = 0.005;
    var sum = 0.0;
    let texel = 1.0 / 2048.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let off = vec2<f32>(f32(dx), f32(dy)) * texel;
            sum = sum + textureSampleCompare(t_shadow, s_shadow, uv + off, proj.z - bias);
        }
    }
    return sum / 9.0;
}

struct VsIn {
    @location(0) pos:     vec3<f32>,
    @location(1) normal:  vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv:      vec2<f32>,
    @location(4) model_c0: vec4<f32>,
    @location(5) model_c1: vec4<f32>,
    @location(6) model_c2: vec4<f32>,
    @location(7) model_c3: vec4<f32>,
    @location(8) tint:       vec4<f32>,
    @location(9) pbr_params: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip:    vec4<f32>,
    @location(0) world_pos:     vec3<f32>,
    @location(1) world_normal:  vec3<f32>,
    @location(2) world_tangent: vec4<f32>,
    @location(3) uv:            vec2<f32>,
    @location(4) tint:          vec4<f32>,
    @location(5) pbr_params:    vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(in.model_c0, in.model_c1, in.model_c2, in.model_c3);
    let world = model * vec4<f32>(in.pos, 1.0);
    out.world_pos = world.xyz;

    let m3 = mat3x3<f32>(model[0].xyz, model[1].xyz, model[2].xyz);
    out.world_normal  = normalize(m3 * in.normal);
    out.world_tangent = vec4<f32>(normalize(m3 * in.tangent.xyz), in.tangent.w);

    out.clip = frame.view_proj * world;
    out.uv = in.uv;
    out.tint = in.tint;
    out.pbr_params = in.pbr_params;
    return out;
}

// ─── BRDF helpers ──────────────────────────────────────────────

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let ndoth = max(dot(n, h), 0.0);
    let ndoth2 = ndoth * ndoth;
    let denom = (ndoth2 * (a2 - 1.0) + 1.0);
    return a2 / max(PI * denom * denom, 1e-7);
}

fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return ndotv / max(ndotv * (1.0 - k) + k, 1e-7);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let ndotv = max(dot(n, v), 0.0);
    let ndotl = max(dot(n, l), 0.0);
    let gv = geometry_schlick_ggx(ndotv, roughness);
    let gl = geometry_schlick_ggx(ndotl, roughness);
    return gv * gl;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    let x = clamp(1.0 - cos_theta, 0.0, 1.0);
    let x5 = x * x * x * x * x;
    return f0 + (vec3<f32>(1.0) - f0) * x5;
}

fn light_dir_and_atten(
    l: LightSlot, world_pos: vec3<f32>,
) -> vec4<f32> {  // xyz=to_light, w=attenuation
    if (l.kind == 0u) {
        return vec4<f32>(normalize(-l.vec), 1.0);
    } else if (l.kind == 1u) {
        let d_vec = l.vec - world_pos;
        let dist = length(d_vec);
        let dir = d_vec / max(dist, 1e-4);
        let r = max(l.range, 1e-4);
        let x = clamp(dist / r, 0.0, 1.0);
        return vec4<f32>(dir, (1.0 - x) * (1.0 - x));
    } else {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let albedo_sample = textureSample(t_albedo, s_albedo, in.uv);
    let albedo = albedo_sample.rgb * in.tint.rgb;
    let metalness = clamp(in.pbr_params.x, 0.0, 1.0);
    let roughness = clamp(in.pbr_params.y, 0.04, 1.0);
    let ao        = clamp(in.pbr_params.z, 0.0, 1.0);
    let has_nmap  = in.pbr_params.w;

    // Reconstruct world-space normal: use normal map if enabled.
    var n = normalize(in.world_normal);
    if (has_nmap > 0.5) {
        let nm = textureSample(t_normal, s_normal, in.uv).xyz * 2.0 - vec3<f32>(1.0);
        let t = normalize(in.world_tangent.xyz);
        let b = normalize(cross(n, t) * in.world_tangent.w);
        let tbn = mat3x3<f32>(t, b, n);
        n = normalize(tbn * nm);
    }

    let v = normalize(frame.eye.xyz - in.world_pos);
    let f0 = mix(vec3<f32>(0.04), albedo, vec3<f32>(metalness));

    let shadow_factor = sample_shadow(in.world_pos);
    var lo = vec3<f32>(0.0);
    var i = 0u;
    loop {
        if (i >= lights.count) { break; }
        if (i >= MAX_LIGHTS) { break; }
        let lr = light_dir_and_atten(lights.lights[i], in.world_pos);
        let l = lr.xyz;
        let atten = lr.w;
        if (atten > 0.0) {
            let h = normalize(l + v);
            let ndotl = max(dot(n, l), 0.0);
            let ndotv = max(dot(n, v), 0.0);

            let d = distribution_ggx(n, h, roughness);
            let g = geometry_smith(n, v, l, roughness);
            let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

            let specular = (d * g * f) / max(4.0 * ndotv * ndotl, 1e-4);
            let kd = (vec3<f32>(1.0) - f) * (1.0 - metalness);
            let diffuse = kd * albedo / PI;

            let radiance = lights.lights[i].color * lights.lights[i].intensity * atten;
            let sf = select(1.0, shadow_factor, i == 0u && lights.lights[i].kind == 0u);
            lo = lo + (diffuse + specular) * radiance * ndotl * sf;
        }
        i = i + 1u;
    }

    let ambient = albedo * lights.ambient * ao;
    var color = ambient + lo;

    // Reinhard tonemap + gamma 2.2
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, albedo_sample.a * in.tint.a);
}
