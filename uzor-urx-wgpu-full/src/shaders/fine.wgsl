// fine.wgsl — Stage 4: per-pixel rect coverage + over-compositing.
//
// Dispatch: dispatch_workgroups(tile_count_x, tile_count_y, 1)
//           workgroup_size(16, 16, 1) → 256 invocations per workgroup.
// Each invocation handles ONE pixel.
// Each workgroup covers ONE 16×16 tile.
//
// Supported CmdKind values:
//   0u = Rect          — solid colour fill
//   2u = LinGradient   — two-colour linear gradient, 4 axis-aligned/diagonal directions
//   3u = RadGradient   — two-colour radial gradient, center+radius from bbox
//   4u = Glyph         — pre-rasterised glyph from R8Unorm atlas, colour-modulated

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
// Glyph atlas: r8unorm single-channel alpha mask.
// Use textureSampleLevel (not textureSample) because compute shaders have
// no implicit derivatives — LOD must be specified explicitly.
@group(0) @binding(5) var glyph_atlas: texture_2d<f32>;
@group(0) @binding(6) var glyph_smp:   sampler;

const TILE_SIZE: u32 = 16u;

/// Unpack a little-endian RGBA8 packed u32 into a normalised vec4<f32>.
/// Byte order: R in bits [0..8], G in [8..16], B in [16..24], A in [24..32].
fn unpack_rgba(packed: u32) -> vec4<f32> {
    let r = f32( packed        & 0xffu) / 255.0;
    let g = f32((packed >>  8u) & 0xffu) / 255.0;
    let b = f32((packed >> 16u) & 0xffu) / 255.0;
    let a = f32((packed >> 24u) & 0xffu) / 255.0;
    return vec4<f32>(r, g, b, a);
}

/// Over-composite src onto (acc_r, acc_g, acc_b, acc_a).
/// src is straight (non-premultiplied) RGBA.
fn over_blend(
    acc_r: f32, acc_g: f32, acc_b: f32, acc_a: f32,
    src: vec4<f32>,
) -> vec4<f32> {
    let sa  = src.a;
    let inv = 1.0 - sa;
    return vec4<f32>(
        src.r * sa + acc_r * inv,
        src.g * sa + acc_g * inv,
        src.b * sa + acc_b * inv,
        sa         + acc_a * inv,
    );
}

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

    let fpx = f32(px);
    let fpy = f32(py);

    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let cmd_idx = tile_lists[base + i];
        let c = cmds[cmd_idx];

        let x0 = c.slot0;
        let y0 = c.slot1;
        let x1 = c.slot2;
        let y1 = c.slot3;

        // Bbox hit test — all cmd kinds share the same bbox slots.
        if (fpx < x0 || fpx >= x1 || fpy < y0 || fpy >= y1) {
            continue;
        }

        var src: vec4<f32>;

        if (c.kind == 0u) {
            // ── Rect: solid colour ──────────────────────────────────────
            src = unpack_rgba(c.slot4);

        } else if (c.kind == 2u) {
            // ── LinGradient ─────────────────────────────────────────────
            // Normalised local coords in [0,1] within bbox.
            let w = x1 - x0;
            let h = y1 - y0;
            var local_x: f32 = 0.0;
            var local_y: f32 = 0.0;
            if (w > 0.0) { local_x = (fpx - x0) / w; }
            if (h > 0.0) { local_y = (fpy - y0) / h; }

            let dir = c.slot6;
            var t: f32;
            if (dir == 0u) {
                // Horizontal: left → right
                t = local_x;
            } else if (dir == 1u) {
                // Vertical: top → bottom
                t = local_y;
            } else if (dir == 2u) {
                // Diagonal TL → BR
                t = (local_x + local_y) * 0.5;
            } else {
                // Diagonal BL → TR
                t = (local_x + (1.0 - local_y)) * 0.5;
            }
            t = clamp(t, 0.0, 1.0);

            let c0 = unpack_rgba(c.slot4); // start color
            let c1 = unpack_rgba(c.slot5); // end color
            src = mix(c0, c1, t);

        } else if (c.kind == 3u) {
            // ── RadGradient ─────────────────────────────────────────────
            let cx = (x0 + x1) * 0.5;
            let cy = (y0 + y1) * 0.5;
            let dx = fpx - cx;
            let dy = fpy - cy;
            let dist = sqrt(dx * dx + dy * dy);
            // max_r = max of half-extents so the gradient reaches all corners.
            let max_r = max((x1 - x0) * 0.5, (y1 - y0) * 0.5);
            var t: f32 = 0.0;
            if (max_r > 0.0) { t = clamp(dist / max_r, 0.0, 1.0); }

            let c0 = unpack_rgba(c.slot4); // inner color
            let c1 = unpack_rgba(c.slot5); // outer color
            src = mix(c0, c1, t);

        } else if (c.kind == 4u) {
            // ── Glyph: alpha from R8Unorm atlas, modulated by colour ─────
            // Compute normalised local coords inside bbox.
            let w = x1 - x0;
            let h = y1 - y0;
            var local_x: f32 = 0.0;
            var local_y: f32 = 0.0;
            if (w > 0.0) { local_x = (fpx - x0) / w; }
            if (h > 0.0) { local_y = (fpy - y0) / h; }
            // Dequantise atlas UV rect from slot5/slot6 (u16 pairs).
            let u0 = f32( c.slot5        & 0xffffu) / 65535.0;
            let v0 = f32((c.slot5 >> 16u) & 0xffffu) / 65535.0;
            let u1 = f32( c.slot6        & 0xffffu) / 65535.0;
            let v1 = f32((c.slot6 >> 16u) & 0xffffu) / 65535.0;
            // Interpolate UV across bbox local coords.
            let u = u0 + local_x * (u1 - u0);
            let v = v0 + local_y * (v1 - v0);
            // Sample atlas alpha.  textureSampleLevel required in compute shaders.
            let alpha = textureSampleLevel(glyph_atlas, glyph_smp, vec2<f32>(u, v), 0.0).r;
            let base = unpack_rgba(c.slot4);
            src = vec4<f32>(base.r, base.g, base.b, base.a * alpha);

        } else {
            // Unknown kind — skip.
            continue;
        }

        // Over-composite src onto accumulator.
        let blended = over_blend(acc_r, acc_g, acc_b, acc_a, src);
        acc_r = blended.r;
        acc_g = blended.g;
        acc_b = blended.b;
        acc_a = blended.a;
    }

    textureStore(output_tex, vec2<i32>(i32(px), i32(py)), vec4<f32>(acc_r, acc_g, acc_b, acc_a));
}
