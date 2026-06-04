// Wave 19 — billboard particle shader.
//
// One triangle strip = 4 verts → quad. Each instance carries
// `pos_size` (xyz=world pos, w=size) and `color`. Vertex shader
// expands the centre into 4 camera-aligned corners.

struct Frame {
    view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up:    vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;

struct VsIn {
    @builtin(vertex_index) v_idx: u32,
    @location(0) pos_size: vec4<f32>,
    @location(1) color:    vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv:    vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    // Quad corner offsets in (right, up) plane for verts 0..3:
    //   0: (-1, -1), 1: (+1, -1), 2: (-1, +1), 3: (+1, +1)
    var off = vec2<f32>(0.0, 0.0);
    switch (in.v_idx) {
        case 0u: { off = vec2<f32>(-1.0, -1.0); }
        case 1u: { off = vec2<f32>( 1.0, -1.0); }
        case 2u: { off = vec2<f32>(-1.0,  1.0); }
        case 3u: { off = vec2<f32>( 1.0,  1.0); }
        default: { off = vec2<f32>(0.0, 0.0); }
    }
    let size = in.pos_size.w;
    let world = in.pos_size.xyz
              + frame.cam_right.xyz * (off.x * size)
              + frame.cam_up.xyz    * (off.y * size);

    var out: VsOut;
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
    out.uv = (off + vec2<f32>(1.0, 1.0)) * 0.5;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Soft radial falloff so each particle looks like a glow.
    let centred = in.uv - vec2<f32>(0.5, 0.5);
    let r2 = dot(centred, centred);
    let alpha = max(0.0, 1.0 - r2 * 4.0); // 0 at r=0.5, 1 at r=0
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
