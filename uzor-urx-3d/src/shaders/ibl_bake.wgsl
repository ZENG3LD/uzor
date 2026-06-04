// Wave 6b — IBL bake shaders. Run as fullscreen triangle render
// passes targeting one cubemap face at a time (the face's NDC quad
// maps to a direction via the standard cube basis).
//
// Three entries:
//   fs_irradiance    — diffuse irradiance (Lambert hemisphere)
//   fs_prefilter     — split-sum specular pre-filter (GGX, per-mip
//                      roughness)
//   fs_brdf_lut      — 2D LUT (ndotv, roughness) → (scale, bias)

const PI: f32 = 3.14159265358979;

@group(0) @binding(0) var t_env: texture_cube<f32>;
@group(0) @binding(1) var s_env: sampler;

struct Params {
    /// X = face index (0..5: +X, -X, +Y, -Y, +Z, -Z)
    /// Y = roughness for the prefilter pass (ignored otherwise)
    /// Z = sample count
    /// W = unused
    args: vec4<f32>,
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
    out.uv = xy; // 0..1, will be remapped to -1..1 for cube direction
    return out;
}

/// Convert (face_index, uv ∈ [0,1]²) to a world-space cube direction.
/// Matches wgpu's cubemap face orientation convention.
fn face_uv_to_dir(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let u = uv.x * 2.0 - 1.0;
    let v = uv.y * 2.0 - 1.0;
    var d = vec3<f32>(0.0);
    switch (face) {
        case 0u: { d = vec3<f32>( 1.0, -v, -u); } // +X
        case 1u: { d = vec3<f32>(-1.0, -v,  u); } // -X
        case 2u: { d = vec3<f32>( u,  1.0,  v); } // +Y
        case 3u: { d = vec3<f32>( u, -1.0, -v); } // -Y
        case 4u: { d = vec3<f32>( u, -v,  1.0); } // +Z
        case 5u: { d = vec3<f32>(-u, -v, -1.0); } // -Z
        default: { d = vec3<f32>(0.0, 0.0, 1.0); }
    }
    return normalize(d);
}

// ─── Hammersley / Importance sampling ────────────────────────────

fn radical_inverse_vdc(in_bits: u32) -> f32 {
    var bits = in_bits;
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return f32(bits) * 2.3283064365386963e-10; // 1 / 2^32
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

/// GGX importance-sampled half-vector around N for given roughness.
fn importance_sample_ggx(xi: vec2<f32>, n: vec3<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;
    let phi = 2.0 * PI * xi.x;
    let cos_t = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
    let sin_t = sqrt(1.0 - cos_t * cos_t);
    let h_local = vec3<f32>(cos(phi) * sin_t, sin(phi) * sin_t, cos_t);
    // Build basis around N.
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let tangent = normalize(cross(up, n));
    let bitan = cross(n, tangent);
    return normalize(tangent * h_local.x + bitan * h_local.y + n * h_local.z);
}

// ─── Diffuse irradiance ──────────────────────────────────────────

@fragment
fn fs_irradiance(in: VsOut) -> @location(0) vec4<f32> {
    let face = u32(params.args.x);
    let n = face_uv_to_dir(face, in.uv);
    // Tangent space around N for hemisphere walk.
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let right = normalize(cross(up, n));
    let bitan = cross(n, right);

    var irradiance = vec3<f32>(0.0);
    let phi_steps = 32u;
    let theta_steps = 16u;
    var samples = 0.0;
    for (var i = 0u; i < phi_steps; i = i + 1u) {
        let phi = (f32(i) / f32(phi_steps)) * 2.0 * PI;
        for (var j = 0u; j < theta_steps; j = j + 1u) {
            let theta = (f32(j) / f32(theta_steps)) * 0.5 * PI;
            let s = sin(theta);
            let c = cos(theta);
            let local = vec3<f32>(cos(phi) * s, sin(phi) * s, c);
            let world = right * local.x + bitan * local.y + n * local.z;
            irradiance += textureSampleLevel(t_env, s_env, world, 0.0).rgb * c * s;
            samples += 1.0;
        }
    }
    irradiance = irradiance * (PI / samples);
    return vec4<f32>(irradiance, 1.0);
}

// ─── Specular pre-filter (GGX) ───────────────────────────────────

@fragment
fn fs_prefilter(in: VsOut) -> @location(0) vec4<f32> {
    let face = u32(params.args.x);
    let roughness = params.args.y;
    let sample_count = u32(params.args.z);
    let n = face_uv_to_dir(face, in.uv);
    let v = n; // assume view = normal (split-sum simplification)
    var total = vec3<f32>(0.0);
    var weight = 0.0;
    for (var i = 0u; i < sample_count; i = i + 1u) {
        let xi = hammersley(i, sample_count);
        let h = importance_sample_ggx(xi, n, roughness);
        let l = normalize(2.0 * dot(v, h) * h - v);
        let ndotl = max(dot(n, l), 0.0);
        if (ndotl > 0.0) {
            total += textureSampleLevel(t_env, s_env, l, 0.0).rgb * ndotl;
            weight += ndotl;
        }
    }
    let color = total / max(weight, 0.001);
    return vec4<f32>(color, 1.0);
}

// ─── 2D BRDF integration LUT ─────────────────────────────────────

fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let a = roughness;
    let k = (a * a) * 0.5;
    return ndotv / (ndotv * (1.0 - k) + k);
}

fn geometry_smith(ndotv: f32, ndotl: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(ndotv, roughness) * geometry_schlick_ggx(ndotl, roughness);
}

@fragment
fn fs_brdf_lut(in: VsOut) -> @location(0) vec4<f32> {
    let ndotv = max(in.uv.x, 0.001);
    let roughness = max(in.uv.y, 0.001);
    let v = vec3<f32>(sqrt(1.0 - ndotv * ndotv), 0.0, ndotv);
    let n = vec3<f32>(0.0, 0.0, 1.0);
    let sample_count = 256u;
    var a = 0.0;
    var b = 0.0;
    for (var i = 0u; i < sample_count; i = i + 1u) {
        let xi = hammersley(i, sample_count);
        let h = importance_sample_ggx(xi, n, roughness);
        let l = normalize(2.0 * dot(v, h) * h - v);
        let ndotl = max(l.z, 0.0);
        let ndoth = max(h.z, 0.0);
        let vdoth = max(dot(v, h), 0.0);
        if (ndotl > 0.0) {
            let g = geometry_smith(ndotv, ndotl, roughness);
            let g_vis = (g * vdoth) / (ndoth * ndotv);
            let fc = pow(1.0 - vdoth, 5.0);
            a += (1.0 - fc) * g_vis;
            b += fc * g_vis;
        }
    }
    a = a / f32(sample_count);
    b = b / f32(sample_count);
    return vec4<f32>(a, b, 0.0, 1.0);
}
