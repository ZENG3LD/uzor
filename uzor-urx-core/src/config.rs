//! `UrxConfig` — process-wide tunable parameters for the render family.
//!
//! Every constant that previously lived as a `const FOO: usize = 256;`
//! inside a backend now has a knob here. Defaults preserve byte-exact
//! output of the 1.4.1 release; consumers don't have to set anything
//! to get the same numbers.
//!
//! ## Design rules
//!
//! - **`Default` matches 1.4.1 behaviour exactly.** No constant changed
//!   value during the introduction of this struct; verified via the
//!   existing `tests/tile_parity.rs` + `tests/simd_parity.rs`.
//! - **Plain data, no Arc.** Cheap to clone (one cache-line + change).
//!   Backends store it by value, so per-frame access is a single load.
//! - **No runtime feature gating logic here.** This struct describes
//!   the policy; the backend decides what to do with it.
//! - **Forward-compatible**: `#[non_exhaustive]` so adding a field is
//!   not a breaking change for consumers using the `Default::default()`
//!   pattern.
//! - **Tile dims validated**: zero or non-multiple-of-4 dims would
//!   break the SIMD assumptions. `validate()` checks them up front
//!   so the backend can panic at *config build* time instead of
//!   silently corrupting tiles later.

/// SIMD aggressiveness level.
///
/// `Native` (default) defers to the `multiversion`-dispatched routines:
/// at startup they detect the host's ISA and pick the best variant
/// (currently AVX2 / NEON / SSE2 / scalar fallback).
///
/// `Scalar` forces the slowest path — useful for parity testing
/// against the SIMD output, for CI on machines where the SIMD ISA
/// isn't available, or for the future `cargo miri` setup which can't
/// run intrinsics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimdLevel {
    /// Force the scalar (non-SIMD) path. Slowest but easiest to audit.
    Scalar,
    /// Up to 128-bit SSE2 (x86) / NEON (ARM). Skip 256-bit.
    Sse2,
    /// Up to 256-bit AVX2.
    Avx2,
    /// Use whatever the runtime CPU dispatch picks (default).
    #[default]
    Native,
}

/// Hybrid-backend dirty-skip strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DirtyStrategy {
    /// Trust consumer-supplied generation counter. Cheapest but
    /// requires the consumer to manually bump on every mutation.
    GenerationOnly,
    /// Hash the pixel bytes each upsert; only re-upload if changed.
    /// Catches consumer bugs at perf cost (~1 GB/s fnv).
    HashBytes,
    /// Both: trust generation but also hash when generation matches
    /// + a `stale` flag is set.
    #[default]
    Both,
}

/// All process-wide tunables for the URX render family.
///
/// Construct via [`UrxConfig::default`] or [`UrxConfig::builder`]
/// (preferred — chainable + names every knob).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct UrxConfig {
    // ── Tile pipeline ──────────────────────────────────────────────

    /// Tile width in pixels for the coarse-tile fast path.
    /// **Must be > 0 and a multiple of 4** (SIMD lane assumption).
    /// Default `32`.
    pub tile_w: u32,
    /// Tile height in pixels. **Must be > 0.** Default `8`.
    pub tile_h: u32,
    /// Minimum tile-row count to engage rayon parallel flush. Below
    /// this the sequential flush dominates due to scheduler overhead.
    /// Default `16`.
    pub parallel_threshold_rows: usize,
    /// Auto-route threshold: scenes with `>= this` commands route to
    /// the tile pipeline (subject to `tile_eligible`); smaller scenes
    /// go straight to scanline. Default `50`.
    pub tile_route_min_cmds: usize,

    // ── LRU caches (CPU) ───────────────────────────────────────────

    /// Gradient LUT cache cap (entries). Default `256`.
    pub gradient_lut_cap: usize,
    /// Rounded-rect mask cache cap (entries). Default `256`.
    pub rounded_mask_cap: usize,
    /// Maximum rounded-rect mask dimension (px). Anything larger is
    /// clamped to this value to avoid runaway allocations. Default
    /// `4096`.
    pub rounded_mask_max_dim: u32,
    /// Glyph atlas LRU cap (cells). Default `1024`.
    pub glyph_cache_cap: usize,

    // ── Engine (retained-mode region cache) ────────────────────────

    /// Soft cap on retained pixmap cache memory (bytes). Default
    /// `64 << 20` (64 MiB).
    pub region_cache_budget_bytes: u64,

    // ── WGPU native pipelines ───────────────────────────────────────

    /// `uzor-urx-wgpu`'s native Path pipeline tessellation-mesh LRU cap
    /// (entries) — see `uzor-urx-wgpu/src/tessellate.rs::TessCache`.
    /// Same hand-rolled-LRU shape as `gradient_lut_cap`/`rounded_mask_cap`
    /// above, just consumed by the WGPU family instead of the CPU one.
    /// Read at `NativeUrxRenderer` construction only — not hot-swappable
    /// (the cache doesn't resize itself mid-session). Wave 1. Default
    /// `256`.
    pub path_tess_cache_cap: usize,
    /// `uzor-urx-wgpu`'s native glyph atlas texture width/height (px) —
    /// see `uzor-urx-wgpu/src/atlas.rs::NativeGlyphAtlas`. Same default
    /// as the legacy crate's proven atlas size
    /// (`uzor-render-wgpu-instanced/src/renderer.rs`'s `ATLAS_SIZE`).
    /// Validated via the same `ConfigError::InvalidAtlasDim` convention
    /// `hybrid_atlas_w/h` already use below (not a new variant — same
    /// "atlas dims must be 1..=16384" rule, a different atlas). Read at
    /// `NativeUrxRenderer` construction only — not hot-swappable (no
    /// atlas-resize path is designed; see `atlas.rs`'s module doc).
    /// Wave 2. Default `2048`.
    pub wgpu_glyph_atlas_w: u32,
    /// See [`Self::wgpu_glyph_atlas_w`]. Wave 2. Default `2048`.
    pub wgpu_glyph_atlas_h: u32,
    /// Maximum blend-layer nesting depth (`PushBlendLayer` without a
    /// matching `PopBlendLayer` yet) — shared symmetrically by BOTH
    /// backends (`uzor-urx-cpu::blend::LayerStack`,
    /// `uzor-urx-wgpu`'s equivalent, one knob not two). Pushes beyond
    /// this cap are suppressed (content keeps drawing into whatever
    /// target was already active, no new offscreen layer allocated) —
    /// bounds the per-nested-layer memory cost (one full-viewport
    /// pixmap/texture per open layer) rather than growing unbounded
    /// for a pathological/buggy scene. Read at backend-construction-
    /// equivalent time (`CpuBackend::render`'s `LayerStack::new`) —
    /// not hot-swappable mid-frame. Wave 3. Default `8`.
    pub blend_layer_max_depth: usize,
    /// `uzor-urx-wgpu`'s native gradient LUT atlas row count — see
    /// `uzor-urx-wgpu/src/gradient_lut.rs::GradientLutAtlas`. The
    /// texture is fixed at 256 columns (matching
    /// `uzor_urx_core::gradient_lut::LUT_SIZE`, one column per LUT
    /// entry); this knob is the OTHER dimension — how many distinct
    /// gradients' LUTs can be resident at once (one per row). Smaller
    /// than the glyph atlas's slot count by design (design §2.2's own
    /// reasoning): distinct SIMULTANEOUS gradients in one frame is
    /// typically far smaller than distinct glyphs. Validated like the
    /// other atlas dims (`ConfigError::InvalidGradientLutRows`) — `0`
    /// rows means every `get_or_insert` fails unconditionally, almost
    /// certainly a config bug rather than an intended "disable" (a
    /// scene with no Radial/Sweep gradient already pays zero cost).
    /// Read at `NativeUrxRenderer` construction only — not hot-
    /// swappable (no atlas-resize path, same as `wgpu_glyph_atlas_w/h`).
    /// Wave 4. Default `64`.
    pub wgpu_gradient_lut_rows: u32,
    /// `uzor-urx-wgpu`'s native per-image texture cache cap (resident
    /// images) — see `uzor-urx-wgpu/src/image_cache.rs::NativeImageCache`.
    /// Count-bound, not area-bound (per design §4.2: "images can be
    /// large — no atlas packing"); each resident image is its own full
    /// `wgpu::Texture`, evicted whole via the same tick-based
    /// never-evict-this-frame LRU discipline as the glyph atlas/
    /// gradient LUT. Read at `NativeUrxRenderer` construction only —
    /// not hot-swappable. Wave 4. Default `64`.
    pub wgpu_image_cache_cap: usize,

    // ── SIMD ───────────────────────────────────────────────────────

    /// Force a specific SIMD level, or `Native` (default) to let the
    /// multiversion runtime dispatch pick.
    pub simd_level: SimdLevel,

    // ── Hybrid backend ─────────────────────────────────────────────

    /// Atlas size for hybrid small-region packing (Hybrid-2). Square,
    /// power-of-two recommended. Default `2048`.
    pub hybrid_atlas_w: u32,
    pub hybrid_atlas_h: u32,
    /// How to decide a region is dirty in hybrid mode.
    pub hybrid_dirty_strategy: DirtyStrategy,
    /// Enable atlas-packed region textures (Hybrid-P1). When `true`,
    /// small regions (each side ≤ `hybrid_atlas_w/2`) are packed into
    /// a shared atlas via the shelf allocator, eliminating per-region
    /// bind group switches. Default `false` — opt-in until rolled out
    /// across consumers (per 16-research configurability directive).
    pub hybrid_atlas_enabled: bool,
    /// Enable single instanced composite draw (Hybrid-P2). Requires
    /// `hybrid_atlas_enabled` to actually batch — without atlas the
    /// per-region texture switches still happen. Default `false`.
    pub hybrid_instanced_composite: bool,
    /// Enable wgpu::PipelineCache disk persistence at backend
    /// construction. The app_id is the consumer's choice; default
    /// "urx" works for single-consumer apps. Default `true` — the
    /// path-key + driver-string-keying means a stale blob silently
    /// falls back to cold compile (fallback: true in create_pipeline_cache).
    pub wgpu_pipeline_cache_enabled: bool,
    /// Pack RGBA into u32 in GPU vertex format (WGPU-P4). When `true`,
    /// the wgpu-instanced backend's vertex shaders consume `u32`
    /// colour fields via `unpack4x8unorm`, saving ~30% of per-instance
    /// upload bandwidth. Default `false` — opt-in because the vertex
    /// format change requires shader recompilation + caller
    /// awareness of the new wire format.
    pub wgpu_packed_color: bool,
    /// Use `var<immediate>` (push constants) for the projection matrix
    /// instead of a uniform buffer (WGPU-P4 partial). Eliminates one
    /// per-frame `write_buffer` + `set_bind_group` call. Default
    /// `false` — opt-in pending verification that the host backend
    /// supports `Features::PUSH_CONSTANTS`.
    pub wgpu_use_immediates_for_projection: bool,
    /// Sort accumulated draw commands by pipeline type before
    /// flushing the encoder (WGPU-P5). Reduces pipeline switch count
    /// from O(N_commands) to O(N_pipeline_types). Default `false` —
    /// behaviour change for painter's order; requires audit that
    /// no cross-type overdraw depends on draw order.
    pub wgpu_sort_by_pipeline: bool,
    /// Use a `wgpu::util::StagingBelt` ring-buffer for per-frame
    /// uploads instead of individual `Queue::write_buffer` calls.
    /// Eliminates the periodic ~25 ms allocation spike (wgpu issue
    /// #1242). Default `false` until belt sizing is calibrated.
    pub wgpu_staging_belt_enabled: bool,
    /// Enable the text-gamma coverage-space adjustment (URX text-gamma
    /// design, 2026-07-26) — `cov' = cov^(1/γ(fg_luma))` applied at
    /// glyph composite time on BOTH backends via the shared
    /// `uzor_urx_core::text_gamma` LUT. Default `false` — opt-in until
    /// the app-fixture vello-diff gate proves it closes the
    /// light-on-dark text-weight gap without moving the CPU-vs-native
    /// byte-tight tier, same rollout discipline as `hybrid_atlas_enabled`/
    /// `wgpu_packed_color` above. Read at `CpuBackend`/`NativeUrxRenderer`
    /// construction only — not hot-swappable (same class of knob as
    /// `wgpu_glyph_atlas_w`/`wgpu_gradient_lut_rows`: baking the LUT
    /// texture is a one-time construction-time cost, not a per-frame
    /// one).
    pub text_gamma_enabled: bool,
}

impl Default for UrxConfig {
    fn default() -> Self {
        Self {
            tile_w: 32,
            tile_h: 8,
            parallel_threshold_rows: 16,
            tile_route_min_cmds: 50,
            gradient_lut_cap: 256,
            rounded_mask_cap: 256,
            rounded_mask_max_dim: 4096,
            glyph_cache_cap: 1024,
            region_cache_budget_bytes: 64 << 20,
            path_tess_cache_cap: 256,
            wgpu_glyph_atlas_w: 2048,
            wgpu_glyph_atlas_h: 2048,
            blend_layer_max_depth: 8,
            wgpu_gradient_lut_rows: 64,
            wgpu_image_cache_cap: 64,
            simd_level: SimdLevel::Native,
            hybrid_atlas_w: 2048,
            hybrid_atlas_h: 2048,
            hybrid_dirty_strategy: DirtyStrategy::Both,
            // B-tier optimisation knobs — default OFF for byte-exact
            // 1.4.x behaviour. Consumers opt in per `16-research
            // compendium`'s 'configurable variant' directive.
            hybrid_atlas_enabled: false,
            hybrid_instanced_composite: false,
            wgpu_pipeline_cache_enabled: true,
            wgpu_packed_color: false,
            wgpu_use_immediates_for_projection: false,
            wgpu_sort_by_pipeline: false,
            wgpu_staging_belt_enabled: false,
            text_gamma_enabled: false,
        }
    }
}

/// Validation error for [`UrxConfig::validate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// `tile_w` was 0 or not a multiple of 4.
    InvalidTileWidth(u32),
    /// `tile_h` was 0.
    InvalidTileHeight(u32),
    /// Atlas dimensions impossible (zero or > 16384).
    InvalidAtlasDim(u32, u32),
    /// `rounded_mask_max_dim` exceeded the safety cap (16384).
    RoundedMaskTooLarge(u32),
    /// `blend_layer_max_depth` was `0` — would suppress EVERY blend
    /// layer unconditionally, almost certainly a bug rather than an
    /// intended "disable" (there is no dedicated disable knob; a
    /// scene that never uses `PushBlendLayer` already pays zero cost).
    InvalidBlendLayerDepth(usize),
    /// `wgpu_gradient_lut_rows` was `0` or exceeded the `1..=16384`
    /// texture-dimension safety cap every other atlas dim in this
    /// struct already enforces.
    InvalidGradientLutRows(u32),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidTileWidth(w) =>
                write!(f, "tile_w must be > 0 and a multiple of 4 (got {})", w),
            ConfigError::InvalidTileHeight(h) =>
                write!(f, "tile_h must be > 0 (got {})", h),
            ConfigError::InvalidAtlasDim(w, h) =>
                write!(f, "hybrid_atlas dims must be 1..=16384 (got {}×{})", w, h),
            ConfigError::RoundedMaskTooLarge(d) =>
                write!(f, "rounded_mask_max_dim must be ≤ 16384 (got {})", d),
            ConfigError::InvalidBlendLayerDepth(d) =>
                write!(f, "blend_layer_max_depth must be > 0 (got {})", d),
            ConfigError::InvalidGradientLutRows(r) =>
                write!(f, "wgpu_gradient_lut_rows must be 1..=16384 (got {})", r),
        }
    }
}

impl std::error::Error for ConfigError {}

impl UrxConfig {
    /// Builder. Start from defaults, override the fields you want.
    pub fn builder() -> UrxConfigBuilder { UrxConfigBuilder::default() }

    /// Validate the config. Call this before passing to any backend.
    /// Backends may also panic at construction time if you skip this.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.tile_w == 0 || self.tile_w % 4 != 0 {
            return Err(ConfigError::InvalidTileWidth(self.tile_w));
        }
        if self.tile_h == 0 {
            return Err(ConfigError::InvalidTileHeight(self.tile_h));
        }
        if self.hybrid_atlas_w == 0 || self.hybrid_atlas_h == 0
            || self.hybrid_atlas_w > 16384 || self.hybrid_atlas_h > 16384
        {
            return Err(ConfigError::InvalidAtlasDim(
                self.hybrid_atlas_w, self.hybrid_atlas_h));
        }
        if self.wgpu_glyph_atlas_w == 0 || self.wgpu_glyph_atlas_h == 0
            || self.wgpu_glyph_atlas_w > 16384 || self.wgpu_glyph_atlas_h > 16384
        {
            return Err(ConfigError::InvalidAtlasDim(
                self.wgpu_glyph_atlas_w, self.wgpu_glyph_atlas_h));
        }
        if self.rounded_mask_max_dim > 16384 {
            return Err(ConfigError::RoundedMaskTooLarge(self.rounded_mask_max_dim));
        }
        if self.blend_layer_max_depth == 0 {
            return Err(ConfigError::InvalidBlendLayerDepth(self.blend_layer_max_depth));
        }
        if self.wgpu_gradient_lut_rows == 0 || self.wgpu_gradient_lut_rows > 16384 {
            return Err(ConfigError::InvalidGradientLutRows(self.wgpu_gradient_lut_rows));
        }
        Ok(())
    }
}

/// Chainable builder for [`UrxConfig`]. Every setter consumes + returns
/// `self`, so you can chain or split however you like.
#[derive(Debug, Clone, Default)]
pub struct UrxConfigBuilder {
    cfg: UrxConfig,
}

macro_rules! setter {
    ($name:ident, $ty:ty) => {
        pub fn $name(mut self, v: $ty) -> Self { self.cfg.$name = v; self }
    };
}

impl UrxConfigBuilder {
    setter!(tile_w, u32);
    setter!(tile_h, u32);
    setter!(parallel_threshold_rows, usize);
    setter!(tile_route_min_cmds, usize);
    setter!(gradient_lut_cap, usize);
    setter!(rounded_mask_cap, usize);
    setter!(rounded_mask_max_dim, u32);
    setter!(glyph_cache_cap, usize);
    setter!(region_cache_budget_bytes, u64);
    setter!(path_tess_cache_cap, usize);
    setter!(wgpu_glyph_atlas_w, u32);
    setter!(wgpu_glyph_atlas_h, u32);
    setter!(blend_layer_max_depth, usize);
    setter!(wgpu_gradient_lut_rows, u32);
    setter!(wgpu_image_cache_cap, usize);
    setter!(simd_level, SimdLevel);
    setter!(hybrid_atlas_w, u32);
    setter!(hybrid_atlas_h, u32);
    setter!(hybrid_dirty_strategy, DirtyStrategy);
    setter!(hybrid_atlas_enabled, bool);
    setter!(hybrid_instanced_composite, bool);
    setter!(wgpu_pipeline_cache_enabled, bool);
    setter!(wgpu_packed_color, bool);
    setter!(wgpu_use_immediates_for_projection, bool);
    setter!(wgpu_sort_by_pipeline, bool);
    setter!(wgpu_staging_belt_enabled, bool);
    setter!(text_gamma_enabled, bool);

    /// Finalise + validate.
    pub fn build(self) -> Result<UrxConfig, ConfigError> {
        self.cfg.validate()?;
        Ok(self.cfg)
    }

    /// Finalise without validation. Use only when you're sure.
    pub fn build_unchecked(self) -> UrxConfig { self.cfg }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_1_4_1_constants() {
        let c = UrxConfig::default();
        assert_eq!(c.tile_w, 32);
        assert_eq!(c.tile_h, 8);
        assert_eq!(c.parallel_threshold_rows, 16);
        assert_eq!(c.tile_route_min_cmds, 50);
        assert_eq!(c.gradient_lut_cap, 256);
        assert_eq!(c.rounded_mask_cap, 256);
        assert_eq!(c.rounded_mask_max_dim, 4096);
        assert_eq!(c.glyph_cache_cap, 1024);
        assert_eq!(c.region_cache_budget_bytes, 64 << 20);
        // Wave-1-introduced (not an original 1.4.1 constant) — default
        // chosen to match the sibling `gradient_lut_cap`/`rounded_mask_cap`
        // convention exactly.
        assert_eq!(c.path_tess_cache_cap, 256);
        // Wave-2-introduced — default matches the legacy crate's proven
        // atlas size.
        assert_eq!(c.wgpu_glyph_atlas_w, 2048);
        assert_eq!(c.wgpu_glyph_atlas_h, 2048);
        // Wave-3-introduced — shared symmetrically by both backends'
        // `LayerStack` equivalents.
        assert_eq!(c.blend_layer_max_depth, 8);
        // Wave-4-introduced — GradientLutAtlas row count / image cache
        // cap, both `uzor-urx-wgpu`-only.
        assert_eq!(c.wgpu_gradient_lut_rows, 64);
        assert_eq!(c.wgpu_image_cache_cap, 64);
        assert_eq!(c.simd_level, SimdLevel::Native);
        assert_eq!(c.hybrid_atlas_w, 2048);
        assert_eq!(c.hybrid_atlas_h, 2048);
        assert_eq!(c.hybrid_dirty_strategy, DirtyStrategy::Both);
        // B-tier opt-in flags — default OFF for byte-exact 1.4.x behaviour.
        assert!(!c.hybrid_atlas_enabled);
        assert!(!c.hybrid_instanced_composite);
        assert!( c.wgpu_pipeline_cache_enabled); // safe default ON — fallback:true
        assert!(!c.wgpu_packed_color);
        assert!(!c.wgpu_use_immediates_for_projection);
        assert!(!c.wgpu_sort_by_pipeline);
        assert!(!c.wgpu_staging_belt_enabled);
        // URX text-gamma design, 2026-07-26, Commit 1 — B-tier opt-in,
        // default OFF (same rollout discipline as every flag above).
        assert!(!c.text_gamma_enabled);
        c.validate().unwrap();
    }

    #[test]
    fn b_tier_flags_can_be_toggled_via_builder() {
        let c = UrxConfig::builder()
            .hybrid_atlas_enabled(true)
            .hybrid_instanced_composite(true)
            .wgpu_packed_color(true)
            .wgpu_use_immediates_for_projection(true)
            .wgpu_sort_by_pipeline(true)
            .wgpu_staging_belt_enabled(true)
            .text_gamma_enabled(true)
            .build()
            .unwrap();
        assert!(c.hybrid_atlas_enabled);
        assert!(c.hybrid_instanced_composite);
        assert!(c.wgpu_packed_color);
        assert!(c.wgpu_use_immediates_for_projection);
        assert!(c.wgpu_sort_by_pipeline);
        assert!(c.wgpu_staging_belt_enabled);
        assert!(c.text_gamma_enabled);
    }

    #[test]
    fn validate_rejects_zero_tile_w() {
        let c = UrxConfig { tile_w: 0, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidTileWidth(0))));
    }

    #[test]
    fn validate_rejects_non_multiple_of_4_tile_w() {
        let c = UrxConfig { tile_w: 33, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidTileWidth(33))));
    }

    #[test]
    fn validate_rejects_zero_tile_h() {
        let c = UrxConfig { tile_h: 0, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidTileHeight(0))));
    }

    #[test]
    fn validate_rejects_oversize_atlas() {
        let c = UrxConfig { hybrid_atlas_w: 32_000, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidAtlasDim(_, _))));
    }

    #[test]
    fn validate_rejects_oversize_glyph_atlas() {
        let c = UrxConfig { wgpu_glyph_atlas_w: 32_000, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidAtlasDim(_, _))));
    }

    #[test]
    fn validate_rejects_zero_glyph_atlas() {
        let c = UrxConfig { wgpu_glyph_atlas_h: 0, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidAtlasDim(_, _))));
    }

    #[test]
    fn validate_rejects_oversize_rounded_mask() {
        let c = UrxConfig { rounded_mask_max_dim: 20_000, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::RoundedMaskTooLarge(20_000))));
    }

    #[test]
    fn validate_rejects_zero_blend_layer_max_depth() {
        let c = UrxConfig { blend_layer_max_depth: 0, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidBlendLayerDepth(0))));
    }

    #[test]
    fn blend_layer_max_depth_settable_via_builder() {
        let c = UrxConfig::builder().blend_layer_max_depth(3).build().unwrap();
        assert_eq!(c.blend_layer_max_depth, 3);
    }

    #[test]
    fn validate_rejects_zero_gradient_lut_rows() {
        let c = UrxConfig { wgpu_gradient_lut_rows: 0, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidGradientLutRows(0))));
    }

    #[test]
    fn validate_rejects_oversize_gradient_lut_rows() {
        let c = UrxConfig { wgpu_gradient_lut_rows: 20_000, ..Default::default() };
        assert!(matches!(c.validate(), Err(ConfigError::InvalidGradientLutRows(20_000))));
    }

    #[test]
    fn gradient_lut_rows_and_image_cache_cap_settable_via_builder() {
        let c = UrxConfig::builder()
            .wgpu_gradient_lut_rows(16)
            .wgpu_image_cache_cap(8)
            .build()
            .unwrap();
        assert_eq!(c.wgpu_gradient_lut_rows, 16);
        assert_eq!(c.wgpu_image_cache_cap, 8);
    }

    #[test]
    fn builder_chains_and_validates() {
        let c = UrxConfig::builder()
            .tile_w(64)
            .tile_h(16)
            .tile_route_min_cmds(100)
            .build()
            .unwrap();
        assert_eq!(c.tile_w, 64);
        assert_eq!(c.tile_h, 16);
        assert_eq!(c.tile_route_min_cmds, 100);
    }

    #[test]
    fn builder_rejects_bad_config_on_build() {
        let r = UrxConfig::builder().tile_w(7).build();
        assert!(r.is_err());
    }
}
