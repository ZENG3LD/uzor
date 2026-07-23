//! URX WGPU backend.
//!
//! Two render paths coexist during the URX Wave 1 cutover
//! (`docs/uzor-engines/plans/urx-wave1-native-pipelines-design-2026-07-25.md`,
//! crate map at `docs/uzor-engines/research/urx-wave1-crate-map-2026-07-25.md`):
//!
//! 1. **Legacy adapter path** (`adapter` module, [`UrxWgpuBackend`]) —
//!    translates a `Scene` into `uzor-render-wgpu-instanced` calls.
//!    Still the production path wired into `uzor-render-hub`; untouched
//!    by Wave 1.
//! 2. **Native pipeline path** ([`NativeUrxRenderer`]) — Wave 1's
//!    self-owned wgpu pipelines (Quad SDF in Commit 1, Line/capsule in
//!    Commit 2, Path/triangle + lyon tessellation in Commit 3) plus
//!    Wave 2's native glyph atlas + Glyph pipeline (Commit 1: atlas,
//!    Commit 2: pipeline + shader + wiring, Commit 3: parity fixture +
//!    text-tolerance budget + doc pass — Wave 2 is now closed),
//!    consuming `Scene` directly with no delegation to the legacy
//!    crate. Exercised by this crate's own tests and the pixel-parity
//!    harness (`tests/parity.rs`, 13/13 green including the glyph
//!    fixture) until the Wave 5 cutover flips production traffic onto it.
//!
//! ## Native pipelines (Wave 1 + Wave 2)
//!
//! [`NativeUrxRenderer`] is the whole of the native path: it owns the
//! Quad SDF (`pipelines::quad`), Line/capsule (`pipelines::line`),
//! Path/triangle (`pipelines::path`), and Glyph (`pipelines::glyph`,
//! sampling `atlas::NativeGlyphAtlas`) `wgpu::RenderPipeline`s, the
//! `ClipStack`-driven `Scene` → instance encoder (`encode`), the lyon
//! tessellation LRU (`tessellate`), and the MSAA offscreen target
//! (`msaa`). Construct via [`NativeUrxRenderer::new`] (defaults) or
//! [`NativeUrxRenderer::with_config`] (accepts a
//! `uzor_urx_core::config::UrxConfig` — `path_tess_cache_cap` and
//! `wgpu_glyph_atlas_w`/`_h` are consumed here); drive one frame via
//! [`NativeUrxRenderer::render_into_encoder`].
//!
//! Two documents are the ground truth for this design, in reading
//! order:
//! - `nemo/docs/uzor-engines/plan-urx-family-parity-2026-07-24.md` —
//!   the wave-scope plan (why URX has 3 sibling backends — CPU/WGPU/
//!   Hybrid — and what "parity" means across them; Wave 1/2's place in
//!   that sequence).
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
//! UrxWgpuBackend::adapt_scene(&scene, &mut ctx);
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
mod msaa;
pub mod native_error;
mod pipelines;
pub mod renderer;
mod shaders;
mod tessellate;

pub use adapter::{adapt_scene_into, UrxWgpuBackend};
pub use atlas::AtlasStats;
pub use native_error::NativeRenderError;
pub use renderer::{NativeUrxRenderer, Viewport};
pub use tessellate::TessCacheStats;

// Re-export the underlying primitive pieces so consumers don't need a
// separate `uzor-render-wgpu-instanced` dep just to call render.
pub use uzor_render_wgpu_instanced::{
    DrawCmd, InstancedRenderContext, InstancedRenderer, LineInstance, QuadInstance,
    TextAreaData, TriangleInstance,
};
