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
//!    text-tolerance budget + doc pass — Wave 2 is now closed), plus
//!    Wave 3's real stencil-based rounded clip + multi-pass blend
//!    layers (Commit 1: CPU blend layers — a different crate, see
//!    `uzor-urx-cpu`; Commit 2: `StencilTarget` + mask-write pipeline +
//!    `ClipStack`'s `Rounded` frame, real `PushClipRoundedRect`; Commit
//!    3: `FrameOp`/`BlendLayerPool`/the multi-pass executor, real
//!    `PushBlendLayer`/`PopBlendLayer`; Commit 4: parity fixtures +
//!    `_CLIP` tolerance tier; Commit 5: this doc pass — Wave 3 is now
//!    closed), consuming `Scene` directly with no delegation to the
//!    legacy crate. Exercised by this crate's own tests and the
//!    pixel-parity harness (`tests/parity.rs`, 16/16 green — 13 from
//!    Waves 1/2 plus Wave 3's 3 clip/blend-layer fixtures) until the
//!    Wave 5 cutover flips production traffic onto it.
//!
//! ## Native pipelines (Wave 1 + Wave 2 + Wave 3)
//!
//! [`NativeUrxRenderer`] is the whole of the native path: it owns the
//! Quad SDF (`pipelines::quad`), Line/capsule (`pipelines::line`),
//! Path/triangle (`pipelines::path`), Glyph (`pipelines::glyph`,
//! sampling `atlas::NativeGlyphAtlas`), stencil mask-write
//! (`pipelines::stencil_mask`), and blend-layer composite
//! (`pipelines::blend_composite`) `wgpu::RenderPipeline`s, the
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
// `GradientLutAtlas`/`NativeImageCache` are greenfield (URX Wave 4
// Commit 2, `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
// §2.2/§4.2) — nothing outside their own unit tests constructs one yet
// (wired into `NativeUrxRenderer`/`encode.rs` in Commit 3), so both
// modules are `#[cfg(test)]`-gated for now, same precedent as Wave 2
// Commit 1's `atlas` module (see that mod's own doc comment above).
#[cfg(test)]
mod gradient_lut;
#[cfg(test)]
mod image_cache;
mod msaa;
pub mod native_error;
mod pipelines;
pub mod renderer;
mod shaders;
mod stencil;
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
