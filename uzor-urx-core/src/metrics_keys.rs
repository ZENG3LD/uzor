//! Flat catalog of all metric KEY_* names emitted by URX backends.
//!
//! Single source of truth. Dashboards, regression alerts, and the
//! `perfwatch`-style live monitors filter by these keys without
//! grepping each crate.
//!
//! Convention: `urx.<phase>.<sub>.<unit>`
//!   `urx.tick.total.us`       — full frame wall-clock
//!   `urx.tick.scene.build.us` — consumer's scene build cost
//!   `urx.render.submit.<backend>.us` — backend submit timing
//!   `urx.render.<phase>.us`   — per-pipeline sub-spans
//!   `urx.region.dirty_count`  — gauge: dirty regions this frame
//!   `urx.cache.hit / miss`    — region-cache counters
//!   `urx.cache.bytes`         — gauge: total cached texture bytes
//!   `urx.cache.evict`         — counter: region textures evicted
//!   `urx.skeleton.frames`     — counter: skeleton frames presented

// ── Tick phases ─────────────────────────────────────────────────────────────

/// Total wall-clock for one render frame (from `engine.render()` entry
/// to return) — μs.
pub const KEY_TICK_TOTAL_US:        &str = "urx.tick.total.us";
/// Consumer-side scene build cost (walker, layout, text shape).
pub const KEY_TICK_SCENE_BUILD_US:  &str = "urx.tick.scene.build.us";
/// Per-region scissored render pass batch — μs (sum across regions).
pub const KEY_TICK_REGION_PASSES_US: &str = "urx.tick.region_passes.us";
/// Final composite pass (blit cached region textures → swap chain).
pub const KEY_TICK_COMPOSITE_US:    &str = "urx.tick.composite.us";
/// `submit_frame` total wall-clock (encode + GPU queue + present).
pub const KEY_TICK_SUBMIT_US:       &str = "urx.tick.submit.us";
/// Counter — windows painted this frame (≥ 1 in multi-window apps).
pub const KEY_TICK_WINDOWS_PAINTED: &str = "urx.tick.windows_painted";
/// Counter — frames rendered since process start.
pub const KEY_TICK_FRAMES:          &str = "urx.tick.frames";

// ── Backend submit timing (one key per backend) ─────────────────────────────

/// Build the `urx.render.submit.<backend>.us` histogram key.
/// `backend` is the `RenderBackend::as_str()` value.
pub fn render_submit_us_key(backend: &str) -> String {
    format!("urx.render.submit.{backend}.us")
}
pub fn render_submit_count_key(backend: &str) -> String {
    format!("urx.render.submit.{backend}.count")
}

// ── Per-pipeline sub-spans (WGPU backend) ───────────────────────────────────

pub const KEY_RENDER_ENCODE_QUADS_US:     &str = "urx.render.encode.quads.us";
pub const KEY_RENDER_ENCODE_LINES_US:     &str = "urx.render.encode.lines.us";
pub const KEY_RENDER_ENCODE_TRIANGLES_US: &str = "urx.render.encode.triangles.us";
pub const KEY_RENDER_ENCODE_TEXT_US:      &str = "urx.render.encode.text.us";
pub const KEY_RENDER_ENCODE_IMAGES_US:    &str = "urx.render.encode.images.us";
pub const KEY_RENDER_R2T_US:              &str = "urx.render.r2t.us";
pub const KEY_RENDER_PRESENT_US:          &str = "urx.render.present.us";

// ── Counts ─────────────────────────────────────────────────────────────────

pub const KEY_RENDER_DRAW_CALLS:    &str = "urx.render.draw_calls";
pub const KEY_RENDER_PRIMITIVES:    &str = "urx.render.primitives";
pub const KEY_RENDER_QUAD_INSTANCES: &str = "urx.render.quad_instances";
pub const KEY_RENDER_LINE_INSTANCES: &str = "urx.render.line_instances";
pub const KEY_RENDER_GLYPH_INSTANCES: &str = "urx.render.glyph_instances";

// ── Per-region cache ────────────────────────────────────────────────────────

pub const KEY_REGION_DIRTY_COUNT:  &str = "urx.region.dirty_count";
pub const KEY_REGION_CLEAN_COUNT:  &str = "urx.region.clean_count";
pub const KEY_REGION_TRANSFORM_ONLY: &str = "urx.region.transform_only_count";

pub const KEY_CACHE_HIT:   &str = "urx.cache.hit";
pub const KEY_CACHE_MISS:  &str = "urx.cache.miss";
pub const KEY_CACHE_BYTES: &str = "urx.cache.bytes";
pub const KEY_CACHE_EVICT: &str = "urx.cache.evict";
pub const KEY_CACHE_COUNT: &str = "urx.cache.count";

// ── Skeleton ────────────────────────────────────────────────────────────────

pub const KEY_SKELETON_FRAMES: &str = "urx.skeleton.frames";
pub const KEY_SKELETON_RENDER_US: &str = "urx.skeleton.render.us";

// ── Text + glyph atlas ──────────────────────────────────────────────────────

pub const KEY_TEXT_SHAPE_US:        &str = "urx.text.shape.us";
pub const KEY_TEXT_RASTER_US:       &str = "urx.text.raster.us";
pub const KEY_TEXT_BUFFER_CACHE_HIT:  &str = "urx.text.buffer_cache.hit";
pub const KEY_TEXT_BUFFER_CACHE_MISS: &str = "urx.text.buffer_cache.miss";
pub const KEY_TEXT_ATLAS_BYTES:     &str = "urx.text.atlas.bytes";

// ── Flat catalog for dashboards ─────────────────────────────────────────────

pub static METRIC_CATALOG: &[&str] = &[
    KEY_TICK_TOTAL_US,
    KEY_TICK_SCENE_BUILD_US,
    KEY_TICK_REGION_PASSES_US,
    KEY_TICK_COMPOSITE_US,
    KEY_TICK_SUBMIT_US,
    KEY_TICK_WINDOWS_PAINTED,
    KEY_TICK_FRAMES,
    KEY_RENDER_ENCODE_QUADS_US,
    KEY_RENDER_ENCODE_LINES_US,
    KEY_RENDER_ENCODE_TRIANGLES_US,
    KEY_RENDER_ENCODE_TEXT_US,
    KEY_RENDER_ENCODE_IMAGES_US,
    KEY_RENDER_R2T_US,
    KEY_RENDER_PRESENT_US,
    KEY_RENDER_DRAW_CALLS,
    KEY_RENDER_PRIMITIVES,
    KEY_RENDER_QUAD_INSTANCES,
    KEY_RENDER_LINE_INSTANCES,
    KEY_RENDER_GLYPH_INSTANCES,
    KEY_REGION_DIRTY_COUNT,
    KEY_REGION_CLEAN_COUNT,
    KEY_REGION_TRANSFORM_ONLY,
    KEY_CACHE_HIT,
    KEY_CACHE_MISS,
    KEY_CACHE_BYTES,
    KEY_CACHE_EVICT,
    KEY_CACHE_COUNT,
    KEY_SKELETON_FRAMES,
    KEY_SKELETON_RENDER_US,
    KEY_TEXT_SHAPE_US,
    KEY_TEXT_RASTER_US,
    KEY_TEXT_BUFFER_CACHE_HIT,
    KEY_TEXT_BUFFER_CACHE_MISS,
    KEY_TEXT_ATLAS_BYTES,
];
