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
//   5u = Stroke        — line segment p0→p1 with scalar width + cap (butt/round/square)
//   6u = Path          — multi-segment polyline; vertices in path_points buffer,
//                        cmd carries bbox + (point_offset, point_count, width)
//   7u = FillPath      — filled closed polygon (non-zero winding); same buffer as Path,
//                        cmd carries bbox + (point_offset, point_count) + colour
//   8u = MultiLinGrad  — N-stop linear gradient; stops packed two-per-vec2 in
//                        path_points (position, bitcast<f32>(packed_rgba))
//   9u = Image         — RGBA8 atlas sample, bbox-local UV, modulated by tint

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
// Binding 7: polyline vertex storage. Path cmds reference a contiguous
// (offset, count) range here; each consecutive pair forms one segment.
@group(0) @binding(7) var<storage, read> path_points: array<vec2<f32>>;
// Binding 8: image atlas (rgba8unorm). Image cmds sample this directly,
// modulating the texel by their tint colour.
@group(0) @binding(8) var image_atlas: texture_2d<f32>;

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

        // Resolve bbox: for Stroke kind=5, derive inflated AABB from
        // endpoints + half-width. For everything else slots 0..3 ARE bbox.
        var x0 = c.slot0;
        var y0 = c.slot1;
        var x1 = c.slot2;
        var y1 = c.slot3;
        var half_w: f32 = 0.0;
        if (c.kind == 5u) {
            half_w = bitcast<f32>(c.slot5) * 0.5;
            x0 = min(c.slot0, c.slot2) - half_w;
            y0 = min(c.slot1, c.slot3) - half_w;
            x1 = max(c.slot0, c.slot2) + half_w;
            y1 = max(c.slot1, c.slot3) + half_w;
        }

        // Bbox hit test (rejects pixels obviously outside the cmd's AABB).
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

        } else if (c.kind == 6u) {
            // ── Path: multi-segment polyline, min-SDF over all segments ─────────
            // slot4 = packed_rgba
            // slot5 low16 = width × 100, high16 = point_count
            // slot6        = point_offset (start index in path_points)
            let width_q = c.slot5 & 0xffffu;
            let count   = (c.slot5 >> 16u) & 0xffffu;
            let offset  = c.slot6;
            let pwidth  = f32(width_q) / 100.0;
            let phw     = pwidth * 0.5;

            // Walk all (count-1) segments, accumulate min distance from
            // pixel to the closest segment. Round-cap behaviour falls out
            // naturally from clamping t to [0, 1].
            let pix = vec2<f32>(fpx, fpy);
            var min_d: f32 = 1.0e9;
            if (count >= 2u) {
                for (var k: u32 = 0u; k < count - 1u; k = k + 1u) {
                    let a = path_points[offset + k];
                    let b = path_points[offset + k + 1u];
                    let ab = b - a;
                    let len2 = dot(ab, ab);
                    var t: f32 = 0.0;
                    if (len2 > 1.0e-6) {
                        t = clamp(dot(pix - a, ab) / len2, 0.0, 1.0);
                    }
                    let q = a + t * ab;
                    let d = distance(pix, q);
                    if (d < min_d) { min_d = d; }
                }
            }
            let coverage = 1.0 - smoothstep(phw - 0.5, phw + 0.5, min_d);
            let pcol = unpack_rgba(c.slot4);
            src = vec4<f32>(pcol.r, pcol.g, pcol.b, pcol.a * coverage);

        } else if (c.kind == 9u) {
            // ── Image: RGBA8 atlas texel × tint colour ──────────────────
            // slot4 = tint rgba (packed)
            // slot5 = atlas UV (u0_q, v0_q) packed u16x2
            // slot6 = atlas UV (u1_q, v1_q) packed u16x2
            let img_w_loc = x1 - x0;
            let img_h_loc = y1 - y0;
            var local_x: f32 = 0.0;
            var local_y: f32 = 0.0;
            if (img_w_loc > 0.0) { local_x = (fpx - x0) / img_w_loc; }
            if (img_h_loc > 0.0) { local_y = (fpy - y0) / img_h_loc; }
            let u0 = f32( c.slot5        & 0xffffu) / 65535.0;
            let v0 = f32((c.slot5 >> 16u) & 0xffffu) / 65535.0;
            let u1 = f32( c.slot6        & 0xffffu) / 65535.0;
            let v1 = f32((c.slot6 >> 16u) & 0xffffu) / 65535.0;
            let u = u0 + local_x * (u1 - u0);
            let v = v0 + local_y * (v1 - v0);
            let texel = textureSampleLevel(image_atlas, glyph_smp, vec2<f32>(u, v), 0.0);
            let tint  = unpack_rgba(c.slot4);
            // Component-wise multiply — tint of (1,1,1,1) gives the
            // atlas colour unchanged.
            src = vec4<f32>(
                texel.r * tint.r,
                texel.g * tint.g,
                texel.b * tint.b,
                texel.a * tint.a,
            );

        } else if (c.kind == 8u) {
            // ── MultiLinGradient: N-stop linear gradient ───────────────
            // slot4 = direction (lin_dir enum)
            // slot5 = stop_count
            // slot6 = stop_offset (into path_points; each stop = vec2<f32>)
            let dir         = c.slot4;
            let stop_count  = c.slot5;
            let stop_offset = c.slot6;

            // Compute t along the gradient direction inside bbox.
            let w = x1 - x0;
            let h = y1 - y0;
            var local_x: f32 = 0.0;
            var local_y: f32 = 0.0;
            if (w > 0.0) { local_x = (fpx - x0) / w; }
            if (h > 0.0) { local_y = (fpy - y0) / h; }
            var t: f32;
            if (dir == 0u) {
                t = local_x;
            } else if (dir == 1u) {
                t = local_y;
            } else if (dir == 2u) {
                t = (local_x + local_y) * 0.5;
            } else {
                t = (local_x + (1.0 - local_y)) * 0.5;
            }
            t = clamp(t, 0.0, 1.0);

            // Walk sorted stops to find the bracketing pair, then lerp.
            // Stop 0 fallback: t < first stop's position → flat first colour.
            // Stop N-1 fallback: t > last stop's position → flat last colour.
            var grad_col: vec4<f32>;
            if (stop_count == 0u) {
                continue;
            } else if (stop_count == 1u) {
                let s0 = path_points[stop_offset];
                grad_col = unpack_rgba(bitcast<u32>(s0.y));
            } else {
                // Default to last stop's colour; broken out below if we
                // find a bracket.
                let last = path_points[stop_offset + stop_count - 1u];
                grad_col = unpack_rgba(bitcast<u32>(last.y));
                let first = path_points[stop_offset];
                if (t <= first.x) {
                    grad_col = unpack_rgba(bitcast<u32>(first.y));
                } else {
                    for (var k: u32 = 0u; k < stop_count - 1u; k = k + 1u) {
                        let s_a = path_points[stop_offset + k];
                        let s_b = path_points[stop_offset + k + 1u];
                        if (t <= s_b.x) {
                            let span = s_b.x - s_a.x;
                            var u_in_span: f32 = 0.0;
                            if (span > 0.0) { u_in_span = (t - s_a.x) / span; }
                            let ca = unpack_rgba(bitcast<u32>(s_a.y));
                            let cb = unpack_rgba(bitcast<u32>(s_b.y));
                            grad_col = mix(ca, cb, clamp(u_in_span, 0.0, 1.0));
                            break;
                        }
                    }
                }
            }
            src = grad_col;

        } else if (c.kind == 7u) {
            // ── FillPath: closed polygon interior, non-zero winding ──
            // slot4 = packed_rgba
            // slot5 = point_count
            // slot6 = point_offset
            //
            // For each edge (v_i, v_{i+1 mod n}): if it crosses the
            // pixel's horizontal ray (going +x), accumulate ±1 to the
            // winding number based on segment orientation. Non-zero
            // winding count → pixel is inside.
            //
            // The polygon implicitly closes: edge (v_{n-1}, v_0) is
            // included.
            let fp_count  = c.slot5;
            let fp_offset = c.slot6;
            var winding: i32 = 0;
            if (fp_count >= 3u) {
                for (var k: u32 = 0u; k < fp_count; k = k + 1u) {
                    let a = path_points[fp_offset + k];
                    let next_idx = select(k + 1u, 0u, k + 1u >= fp_count);
                    let b = path_points[fp_offset + next_idx];
                    // Half-open edge test: an edge is counted iff
                    //   (a.y <= py < b.y) OR (b.y <= py < a.y)
                    // — avoids double-counting at shared vertices.
                    let cond_up   = (a.y <= fpy) && (b.y >  fpy);
                    let cond_down = (b.y <= fpy) && (a.y >  fpy);
                    if (cond_up || cond_down) {
                        // x intersection of segment with horizontal ray y = fpy.
                        let t  = (fpy - a.y) / (b.y - a.y);
                        let xi = a.x + t * (b.x - a.x);
                        if (xi > fpx) {
                            if (cond_up)   { winding = winding + 1; }
                            if (cond_down) { winding = winding - 1; }
                        }
                    }
                }
            }
            let inside = winding != 0;
            let fcol = unpack_rgba(c.slot4);
            // Solid in/out — no AA on the polygon edge for v1. AA via
            // sub-pixel sampling can be added later by averaging a 2×2
            // or 4×4 sample grid; current approach prefers speed +
            // determinism over edge smoothness.
            if (inside) {
                src = fcol;
            } else {
                continue;
            }

        } else if (c.kind == 5u) {
            // ── Stroke: signed-distance line segment with cap-aware coverage ──
            // Endpoints come straight from slot0..slot3 (NOT bbox).
            let p0  = vec2<f32>(c.slot0, c.slot1);
            let p1  = vec2<f32>(c.slot2, c.slot3);
            let pix = vec2<f32>(fpx, fpy);
            let ab  = p1 - p0;
            let len2 = dot(ab, ab);
            let cap_kind = c.slot6 & 0xffu;

            // Project pix onto the infinite line through (p0, p1).
            // t in [0,1] = inside segment; <0 or >1 = past an endpoint.
            var t_raw: f32 = 0.0;
            if (len2 > 1e-6) {
                t_raw = dot(pix - p0, ab) / len2;
            }
            // Capsule SDF: clamp t to segment, dist = |pix - lerp(p0,p1,t)|.
            // Round + butt + square caps all share this clamp; the
            // *coverage* differs at endpoints by how we widen t's domain.
            var t: f32;
            if (cap_kind == 2u) {
                // SQUARE: extend the segment by half_w/length(ab) past each end.
                let len_inv = inverseSqrt(max(len2, 1e-6));
                let ext = half_w * len_inv;
                t = clamp(t_raw, -ext, 1.0 + ext);
            } else {
                // BUTT (0) + ROUND (1): clamp t to [0, 1].
                t = clamp(t_raw, 0.0, 1.0);
            }
            let q   = p0 + t * ab;
            let d   = distance(pix, q);

            // Coverage: 1 inside (d < half_w - 0.5), 0 outside (d > half_w + 0.5),
            // linear AA across the 1-pixel transition band.
            //
            // BUTT cap is a special case — pixels with t_raw outside [0,1]
            // must be fully transparent regardless of d (no perpendicular
            // bleed past the line ends). Round (cap_kind=1) lets them
            // through (semicircle); Square clamps the extended t above.
            var coverage: f32 = 1.0 - smoothstep(half_w - 0.5, half_w + 0.5, d);
            if (cap_kind == 0u && (t_raw < 0.0 || t_raw > 1.0)) {
                coverage = 0.0;
            }

            let base_col = unpack_rgba(c.slot4);
            src = vec4<f32>(base_col.r, base_col.g, base_col.b, base_col.a * coverage);

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
