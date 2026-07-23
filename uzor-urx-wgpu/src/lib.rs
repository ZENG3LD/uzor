//! URX WGPU backend.
//!
//! Two render paths coexist during the URX Wave 1 cutover
//! (`docs/uzor-engines/plans/urx-wave1-native-pipelines-design-2026-07-25.md`,
//! crate map at `docs/uzor-engines/research/urx-wave1-crate-map-2026-07-25.md`):
//!
//! 1. **Legacy adapter path** (`adapter` module) — translates a `Scene`
//!    into `uzor-render-wgpu-instanced` calls via the free fn
//!    [`adapt_scene_into`]. No longer `uzor-render-hub`'s production
//!    path for the ordinary (autodetect-reachable) 2D submit as of Wave
//!    6 Commit 1 (`submit_urx_wgpu` now renders through
//!    [`NativeUrxRenderer`] — see below); `adapt_scene_into` itself
//!    stays live for `uzor-urx-engine`'s retained-mode `engine.rs`
//!    (`uzor-render-hub`'s own `compose_urx_wgpu_into_swap` cut over to
//!    the native renderer in the Wave 7 tail, 2026-07-24 — see
//!    `uzor-render-hub::compose`'s own module doc for that call site's
//!    current shape). The `UrxWgpuBackend` marker struct that used to
//!    sit alongside this free fn had zero real call sites (only a doc
//!    example + a `#[allow(dead_code)]`-marked `WindowRenderState` field)
//!    and was removed in that same pass — this module now exports only
//!    the free fn.
//! 2. **Native pipeline path** ([`NativeUrxRenderer`]) — Wave 1's
//!    self-owned wgpu pipelines (Quad SDF in Commit 1, Line/capsule in
//!    Commit 2, Path/triangle + lyon tessellation in Commit 3) plus
//!    Wave 2's native glyph atlas + Glyph pipeline (Commit 1: atlas,
//!    Commit 2: pipeline + shader + wiring, Commit 3: parity fixture +
//!    text-tolerance budget + doc pass — Wave 2 is now closed), plus
//!    Wave 3's real stencil-based rounded clip + multi-pass blend
//!    layers (Commit 1: CPU blend layers — a different crate, see
//!    `uzor-urx-cpu`; Commit 2: `StencilTarget` + mask-write pipeline +
//!    `ClipStack`'s `Rounded` frame, real `PushClipRoundedRect`; Commit
//!    3: `FrameOp`/`BlendLayerPool`/the multi-pass executor, real
//!    `PushBlendLayer`/`PopBlendLayer`; Commit 4: parity fixtures +
//!    `_CLIP` tolerance tier; Commit 5: this doc pass — Wave 3 is now
//!    closed), plus Wave 4's Radial/Sweep gradients, images, full
//!    affine, and per-corner radii (Commit 1: CPU gradient fixes +
//!    shared-crate extractions — a different crate/a new crate, see
//!    `uzor-urx-cpu`/`uzor-urx-image`; Commit 2: `GradientLutAtlas`/
//!    `NativeImageCache` infra; Commit 3: `GradientPipeline`/
//!    `ImagePipeline` + `encode.rs` wiring; Commit 4: full affine +
//!    stroke-width unification + per-corner radii routing; Commits
//!    5+6: 6 new parity fixtures + 3 GPU-only correctness tests + this
//!    doc pass — Wave 4 is now closed), consuming `Scene` directly with
//!    no delegation to the legacy crate. Exercised by this crate's own
//!    tests and the pixel-parity harness (`tests/parity.rs`, 22/22
//!    CPU-vs-GPU cases green — 16 from Waves 1-3 plus Wave 4's 6 new
//!    ones — plus 3 GPU-only correctness tests that have no CPU
//!    baseline to compare against, design §0.3). Production traffic:
//!    Wave 5 flipped `uzor-render-hub::compose.rs`'s Phase 3 (chrome)
//!    and Phase 4.5 (post-3D overlay) onto this path; Wave 6 Commit 1
//!    (`urx-wave6-autodetect-cutover-design-2026-07-25.md` §3) flipped
//!    the ordinary (non-compose) `submit_urx_wgpu` path too — the exact
//!    function `uzor-render-hub::detect_backend`'s future GPU-autodetect
//!    arm will make live. Both cutovers share ONE per-window
//!    `NativeUrxRenderer` instance (`WindowRenderState.urx_native_renderer`).
//!
//! ## Native pipelines (Wave 1 + Wave 2 + Wave 3 + Wave 4)
//!
//! [`NativeUrxRenderer`] is the whole of the native path: it owns the
//! Quad SDF (`pipelines::quad`, rotation-capable since Wave 4 Commit 4),
//! Line/capsule (`pipelines::line`), Path/triangle (`pipelines::path`),
//! Glyph (`pipelines::glyph`, sampling `atlas::NativeGlyphAtlas`),
//! stencil mask-write (`pipelines::stencil_mask`), blend-layer composite
//! (`pipelines::blend_composite`), Radial/Sweep Gradient
//! (`pipelines::gradient`, sampling `gradient_lut::GradientLutAtlas`),
//! and Image (`pipelines::image`, sampling
//! `image_cache::NativeImageCache`) `wgpu::RenderPipeline`s, the
//! `ClipStack`-driven `Scene` → op-list encoder (`encode` —
//! `FrameOp::{Draw,PushLayer,PopLayer}` since Wave 3 Commit 3, not a
//! flat batch list), the lyon tessellation LRU (`tessellate`), the MSAA
//! offscreen target (`msaa`), the root `StencilTarget` (`stencil`), and
//! the depth-indexed `BlendLayerPool` (`renderer`). Construct via
//! [`NativeUrxRenderer::new`] (defaults) or
//! [`NativeUrxRenderer::with_config`] (accepts a
//! `uzor_urx_core::config::UrxConfig` — `path_tess_cache_cap`,
//! `wgpu_glyph_atlas_w`/`_h`, and `blend_layer_max_depth` are consumed
//! here); drive one frame via
//! [`NativeUrxRenderer::render_into_encoder`], which opens exactly one
//! `wgpu::RenderPass` for a scene with no blend layers (byte-identical
//! to Waves 1/2) or the multi-pass executor (`renderer::replay_ops`)
//! for one that has them.
//!
//! Three documents are the ground truth for this design, in reading
//! order:
//! - `nemo/docs/uzor-engines/plan-urx-family-parity-2026-07-24.md` —
//!   the wave-scope plan (why URX has 3 sibling backends — CPU/WGPU/
//!   Hybrid — and what "parity" means across them; each wave's place
//!   in that sequence).
//! - `nemo/docs/uzor-engines/plans/urx-wave1-native-pipelines-design-2026-07-25.md` —
//!   Wave 1's design: module layout, instance struct layouts, the
//!   AA-scheme decision (SDF for Quad/Line, MSAA-only — no
//!   barycentric edge AA — for Path), the pixel-parity harness spec,
//!   and the 5-commit plan Wave 1 was built across (each commit's own
//!   module docs cite the specific design section it implements).
//! - `nemo/docs/uzor-engines/plans/urx-wave2-native-glyph-atlas-design-2026-07-25.md` —
//!   Wave 2's design: the native glyph atlas (`atlas::NativeGlyphAtlas`,
//!   `etagere`-backed, never-evict-this-frame invariant), the Glyph
//!   pipeline, and the 3-commit plan Wave 2 is built across.
//! - `nemo/docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md` —
//!   Wave 3's design: real stencil-based rounded clip (§2), the
//!   multi-pass blend-layer executor (§3, including its pass-open/close
//!   load-bearing table, §3.5), the CPU-side blend-layer stack (§5, a
//!   different crate — see `uzor-urx-cpu`), the 3 new parity fixtures
//!   + `_CLIP` tolerance tier (§6/§2.6), and the 5-commit plan Wave 3
//!   was built across.
//! - `nemo/docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md` —
//!   Wave 4's design: Radial/Sweep gradients (§2, per-fragment LUT
//!   eval), images (§4, `DrawCommand::Image` + the shared
//!   `uzor-urx-image` registry crate), full affine (§5,
//!   `decompose_similarity`'s Quad-SDF-vs-Triangle routing + full
//!   6-coefficient mesh reprojection), per-corner radii (§6, closes
//!   `native_per_corner_radii_uniform_approx`), the 7 new parity
//!   fixtures + `_GRADIENT`/`_IMAGE` tolerance tiers (§9), and the
//!   6-commit plan Wave 4 was built across.
//!
//! File:line evidence for every legacy pattern this design reuses
//! (device/queue ownership shape, MSAA lifecycle, hand-rolled LRU
//! shape, etc.) lives in the sibling research doc:
//! `nemo/docs/uzor-engines/research/urx-wave1-crate-map-2026-07-25.md`.
//!
//! ## Legacy API
//!
//! ```ignore
//! let mut ctx = InstancedRenderContext::new(w, h, 0.0, 0.0);
//! adapt_scene_into(&scene, &mut ctx);
//! // -> caller hands `ctx.draw_commands` to InstancedRenderer::render
//! //    (or via uzor-render-hub's submit_instanced)
//! ```

pub mod adapter;
// `NativeGlyphAtlas` is now constructed by real (non-test) code —
// `renderer.rs`/`pipelines/glyph.rs` wire it into `NativeUrxRenderer`
// as of Wave 2 Commit 2 — so the `#[cfg(test)]` gate Commit 1 needed
// (nothing but this module's own unit tests constructed one yet) is
// gone.
mod atlas;
mod encode;
// `GradientLutAtlas`/`NativeImageCache` were greenfield in URX Wave 4
// Commit 2 (`docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
// §2.2/§4.2, `#[cfg(test)]`-gated then, same precedent as Wave 2
// Commit 1's `atlas` module) — Commit 3 wires both into
// `NativeUrxRenderer`/`encode.rs` for real, so the gate is lifted here,
// same as `atlas`'s own history above.
mod gradient_lut;
mod image_cache;
mod msaa;
pub mod native_error;
mod pipelines;
pub mod renderer;
mod shaders;
mod stencil;
mod tessellate;

pub use adapter::adapt_scene_into;
pub use atlas::AtlasStats;
pub use gradient_lut::GradientLutAtlasStats;
pub use image_cache::NativeImageCacheStats;
pub use native_error::NativeRenderError;
pub use renderer::{NativeUrxRenderer, Viewport};
pub use tessellate::TessCacheStats;

// Re-export the underlying primitive pieces so consumers don't need a
// separate `uzor-render-wgpu-instanced` dep just to call render.
pub use uzor_render_wgpu_instanced::{
    DrawCmd, InstancedRenderContext, InstancedRenderer, LineInstance, QuadInstance,
    TextAreaData, TriangleInstance,
};
