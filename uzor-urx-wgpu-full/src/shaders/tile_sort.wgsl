struct Uniforms {
    cmd_count:    u32,
    tile_count_x: u32,
    tile_count_y: u32,
    tile_cmd_cap: u32,
};

@group(0) @binding(0) var<uniform> uni: Uniforms;
@group(0) @binding(2) var<storage, read_write> tile_counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> tile_lists: array<u32>;

// Note: binding 1 (cmds) is unused here but must match the BGL layout
// — declare a dummy storage to keep the BGL slot occupied.
struct SceneCmd {
    kind:  u32,
    slot0: f32, slot1: f32, slot2: f32, slot3: f32,
    slot4: u32, slot5: u32, slot6: u32,
};
@group(0) @binding(1) var<storage, read> cmds: array<SceneCmd>;

@compute @workgroup_size(64)
fn sort(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tile_id = gid.x;
    let total_tiles = uni.tile_count_x * uni.tile_count_y;
    if (tile_id >= total_tiles) { return; }

    let raw_n = atomicLoad(&tile_counts[tile_id]);
    let n = min(raw_n, uni.tile_cmd_cap);
    if (n < 2u) { return; }
    let base = tile_id * uni.tile_cmd_cap;

    // Insertion sort by cmd index — preserves painter's order since
    // encoder emitted cmds in painter order (lower index = drawn first).
    for (var i: u32 = 1u; i < n; i = i + 1u) {
        let key = tile_lists[base + i];
        var j: u32 = i;
        loop {
            if (j == 0u) { break; }
            let prev = tile_lists[base + j - 1u];
            if (prev <= key) { break; }
            tile_lists[base + j] = prev;
            j = j - 1u;
        }
        tile_lists[base + j] = key;
    }
}
