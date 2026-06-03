// fine.wgsl — Stage 4: per-pixel rect coverage + over-compositing.
//
// Dispatch: dispatch_workgroups(tile_count_x, tile_count_y, 1)
//           workgroup_size(16, 16, 1) → 256 invocations per workgroup.
// Each invocation handles ONE pixel.
// Each workgroup covers ONE 16×16 tile.
//
// Coarse pass note (v1.6.0 rect-only):
//   The sorted tile_lists buffer IS the PTCL for rect-only scenes —
//   no separate coarse.wgsl needed. Fine reads tile_lists directly.
//   Add coarse.wgsl when implementing gradient/glyph variants.

struct SceneCmd {
    kind:  u32,
    slot0: f32, slot1: f32, slot2: f32, slot3: f32,
    slot4: u32, slot5: u32, slot6: u32,
};

struct Uniforms {
    cmd_count:    u32,
    tile_count_x: u32,
    tile_count_y: u32,
    tile_cmd_cap: u32,
};

@group(0) @binding(0) var<uniform>              uni:         Uniforms;
@group(0) @binding(1) var<storage, read>        cmds:        array<SceneCmd>;
// tile_counts is read-only in fine pass, but BGL declares it as
// read_write (needed by tile_assign atomics) — WGSL access must match,
// so declare read_write here even though we only load. We don't need
// atomic semantics for the read; plain index is fine.
@group(0) @binding(2) var<storage, read_write>  tile_counts: array<u32>;
@group(0) @binding(3) var<storage, read_write>  tile_lists:  array<u32>;
@group(0) @binding(4) var                       output_tex:  texture_storage_2d<rgba8unorm, write>;

const TILE_SIZE: u32 = 16u;

@compute @workgroup_size(16, 16, 1)
fn fine(
    @builtin(workgroup_id)           wg:  vec3<u32>,
    @builtin(local_invocation_id)    lid: vec3<u32>,
) {
    // Pixel coords in screen space.
    let px = wg.x * TILE_SIZE + lid.x;
    let py = wg.y * TILE_SIZE + lid.y;

    // Bounds-check: viewport may not be a multiple of tile size.
    let screen_w = uni.tile_count_x * TILE_SIZE;
    let screen_h = uni.tile_count_y * TILE_SIZE;
    if (px >= screen_w || py >= screen_h) {
        return;
    }

    let tile_id = wg.y * uni.tile_count_x + wg.x;
    let raw_n   = tile_counts[tile_id];
    let n       = min(raw_n, uni.tile_cmd_cap);
    let base    = tile_id * uni.tile_cmd_cap;

    // Accumulate over-blend: premultiplied alpha compositing.
    var acc_r: f32 = 0.0;
    var acc_g: f32 = 0.0;
    var acc_b: f32 = 0.0;
    var acc_a: f32 = 0.0;

    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let cmd_idx = tile_lists[base + i];
        let c = cmds[cmd_idx];

        if (c.kind != 0u) { continue; } // rect only in v1.6.0

        let x0 = c.slot0;
        let y0 = c.slot1;
        let x1 = c.slot2;
        let y1 = c.slot3;

        let fpx = f32(px);
        let fpy = f32(py);

        if (fpx >= x0 && fpx < x1 && fpy >= y0 && fpy < y1) {
            // Unpack RGBA from slot4 (RGBA little-endian packed u32).
            let packed = c.slot4;
            let sr = f32( packed        & 0xffu) / 255.0;
            let sg = f32((packed >>  8u) & 0xffu) / 255.0;
            let sb = f32((packed >> 16u) & 0xffu) / 255.0;
            let sa = f32((packed >> 24u) & 0xffu) / 255.0;

            // Over-operator (premultiplied):
            //   dst = src*sa + dst*(1-sa)
            let inv = 1.0 - sa;
            acc_r = sr * sa + acc_r * inv;
            acc_g = sg * sa + acc_g * inv;
            acc_b = sb * sa + acc_b * inv;
            acc_a = sa       + acc_a * inv;
        }
    }

    textureStore(output_tex, vec2<i32>(i32(px), i32(py)), vec4<f32>(acc_r, acc_g, acc_b, acc_a));
}
