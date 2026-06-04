// Wave 20 — depth-based screen-space ambient occlusion (cheap).
//
// For each output pixel:
//   1. Read centre depth (Depth32Float, native [0,1] device range).
//   2. Linearize via the camera's near/far (passed in `params.xy`).
//   3. Sample N neighbors in a screen-space disc of radius
//      `params.z` (pixels). For each neighbor, linearize its depth
//      and ask: is the neighbor noticeably CLOSER than the centre?
//      If yes (delta in [0, params.w] world units) → it occludes.
//   4. Occlusion = clamp(occluder_count / N, 0, 1); output AO = 1 - occlusion.
//
// Cheap because it doesn't reconstruct world normals — only relative
// depth differences. Looks great in cracks/corners; doesn't darken
// open surfaces.

@group(0) @binding(0) var t_depth: texture_depth_2d;
@group(0) @binding(1) var s_depth: sampler;
@group(0) @binding(2) var<uniform> params: vec4<f32>;
// params.x = near
// params.y = far
// params.z = sample radius (pixels)
// params.w = max delta (world units) — neighbor counts as occluder only
//            if its depth is between [centre - max_delta, centre]

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

fn linearize(depth: f32, near: f32, far: f32) -> f32 {
    // wgpu RH projection: depth in [0, 1] with 0 = near plane.
    // Reverse the standard mapping: z_view = -near * far / (depth * (near - far) + far)
    return near * far / (far - depth * (far - near));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let near = params.x;
    let far  = params.y;
    let radius_px = params.z;
    let max_delta = params.w;

    let dims = vec2<f32>(textureDimensions(t_depth, 0));
    let inv_dims = 1.0 / dims;

    let centre_depth = textureSample(t_depth, s_depth, in.uv);
    if (centre_depth >= 1.0) {
        // Sky / far plane — no occlusion.
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    let centre_lin = linearize(centre_depth, near, far);

    // 12 pre-baked unit-disc samples for a Poisson-ish ring.
    let samples = array<vec2<f32>, 12>(
        vec2<f32>( 0.80,  0.00),
        vec2<f32>( 0.40,  0.70),
        vec2<f32>(-0.40,  0.70),
        vec2<f32>(-0.80,  0.00),
        vec2<f32>(-0.40, -0.70),
        vec2<f32>( 0.40, -0.70),
        vec2<f32>( 0.30,  0.20),
        vec2<f32>(-0.30,  0.20),
        vec2<f32>( 0.00, -0.30),
        vec2<f32>( 0.55, -0.40),
        vec2<f32>(-0.55, -0.40),
        vec2<f32>( 0.00,  0.45),
    );

    var occluders = 0.0;
    for (var i = 0u; i < 12u; i = i + 1u) {
        let off = samples[i] * radius_px * inv_dims;
        let d = textureSample(t_depth, s_depth, in.uv + off);
        if (d >= 1.0) { continue; } // sky tap
        let d_lin = linearize(d, near, far);
        let delta = centre_lin - d_lin;
        if (delta > 0.0 && delta < max_delta) {
            // Falloff: linearly fades occluder weight to 0 at max_delta.
            let w = 1.0 - delta / max_delta;
            occluders += w;
        }
    }
    let occ = clamp(occluders / 12.0, 0.0, 1.0);
    let ao = 1.0 - occ;
    return vec4<f32>(ao, ao, ao, 1.0);
}
