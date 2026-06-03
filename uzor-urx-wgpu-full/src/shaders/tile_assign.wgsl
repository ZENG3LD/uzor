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

@group(0) @binding(0) var<uniform> uni: Uniforms;
@group(0) @binding(1) var<storage, read> cmds: array<SceneCmd>;
@group(0) @binding(2) var<storage, read_write> tile_counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> tile_lists: array<u32>;
// Binding 4: output texture — unused in assign, declared to satisfy shared BGL.
@group(0) @binding(4) var output_tex: texture_storage_2d<rgba8unorm, write>;
// Bindings 5+6: glyph atlas + sampler — unused in assign, declared to satisfy shared BGL.
@group(0) @binding(5) var glyph_atlas: texture_2d<f32>;
@group(0) @binding(6) var glyph_smp:   sampler;

const TILE_SIZE: u32 = 16u;

@compute @workgroup_size(64)
fn assign(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= uni.cmd_count) { return; }
    let c = cmds[i];
    // All cmd kinds that occupy a bbox participate in tile binning:
    //   kind 0 = Rect, 2 = LinGradient, 3 = RadGradient
    // Unknown / reserved kinds (1) skip.
    if (c.kind == 1u) { return; }

    let x0 = max(0.0, c.slot0);
    let y0 = max(0.0, c.slot1);
    let x1 = max(x0, c.slot2);
    let y1 = max(y0, c.slot3);

    let tx_min = u32(x0) / TILE_SIZE;
    let ty_min = u32(y0) / TILE_SIZE;
    let tx_max = u32(x1) / TILE_SIZE;
    let ty_max = u32(y1) / TILE_SIZE;

    let tx_lo = min(tx_min, uni.tile_count_x - 1u);
    let ty_lo = min(ty_min, uni.tile_count_y - 1u);
    let tx_hi = min(tx_max, uni.tile_count_x - 1u);
    let ty_hi = min(ty_max, uni.tile_count_y - 1u);

    for (var ty: u32 = ty_lo; ty <= ty_hi; ty = ty + 1u) {
        for (var tx: u32 = tx_lo; tx <= tx_hi; tx = tx + 1u) {
            let tile_id = ty * uni.tile_count_x + tx;
            let slot = atomicAdd(&tile_counts[tile_id], 1u);
            if (slot < uni.tile_cmd_cap) {
                tile_lists[tile_id * uni.tile_cmd_cap + slot] = i;
            }
            // else: overflow — caller can detect via tile_counts > cap
        }
    }
}
