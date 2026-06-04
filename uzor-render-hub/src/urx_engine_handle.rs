//! `UrxEngineHandle` — fat handle for the URX channel.
//!
//! The 2026-06-05 dual-enum design (see
//! `uzor-tessera/docs/plans/urx-full-integration-2026-06-05.md`) splits
//! the catalog into two parallel channels:
//!
//! * `with_render_context` — legacy 2D scene channel. Goes through
//!   `Scene2DBackend`. Vello / tiny-skia / canvas2d consumers stay 1:1
//!   with what they had before the split.
//! * `with_urx_engine` — new URX channel. Goes through `UrxBackend`.
//!   Exposes the FULL URX-family surface via `UrxEngineHandle`: 2D
//!   Scene IR + retained-mode regions (Stage 3) + 3D scene (Stage 4)
//!   + particles (Stage 4) + physics handle (Stage 4) + skeleton
//!   frame (Stage 5).
//!
//! Stage 1 ships only the `render_ctx` field. The remaining fields
//! (`engine`, `r3d`, `scene_3d`, `physics`, `particles`) are `Option`
//! slots that are always `None` for now — they light up in their
//! respective stages.
//!
//! Consumers who only paint primitives use `h.render_ctx` and ignore
//! the rest, identical to the legacy `with_render_context` ergonomics.

use uzor::render::RenderContext;
use uzor_urx_engine::UrxEngine;

/// Fat handle handed to the closure passed to `RenderHub::with_urx_engine`.
///
/// Lifetime `'a` binds to the borrow of the `WindowRenderState` taken
/// for the duration of the callback. All fields are mutable borrows —
/// the consumer may freely mutate any subset.
///
/// Consumer modes:
/// * **Immediate** — use only `render_ctx`. Ergonomics identical to the
///   legacy `with_render_context` channel; the Scene buffered by
///   `UrxRenderContext` is handed to the active URX backend at submit
///   time as one big region.
/// * **Retained** — drive `engine` (upsert_region / mark_dirty /
///   needs_paint / render). Multiple regions get independent dirty
///   tracking + per-region BackendHint dispatch. Stage 3 surface; the
///   default `RegionMixer` wire to submit lands in Stage 3 part 2.
pub struct UrxEngineHandle<'a> {
    /// Standard 2D scene (Canvas2D shape — fill_rect/stroke/text/...).
    /// Same `RenderContext` trait the legacy channel uses, so a
    /// consumer that ignores the rest of this handle still draws
    /// pixels with one line.
    pub render_ctx: &'a mut dyn RenderContext,

    /// Retained-mode engine handle. Always `Some` in Stage 3+ — lazy-
    /// initialised on the first `with_urx_engine` call with the URX
    /// channel armed. Consumers that only use the immediate path can
    /// ignore this field; consumers that want regions call methods on
    /// it directly.
    pub engine: &'a mut UrxEngine,

    /// Frame dimensions in physical pixels (post-DPI scaling).
    pub width: u32,
    pub height: u32,
    /// Device pixel ratio in effect.
    pub dpr: f64,
    /// Monotonic frame counter for this window.
    pub frame_idx: u64,
    // ── Stage 4+ fields (currently absent) ────────────────────────────────
    //
    // Stage 4 will add:
    //   * `pub r3d: Option<&'a mut Renderer3D>`
    //   * `pub scene_3d: Option<&'a mut Scene3D>`
    //   * `pub physics: Option<&'a mut PhysicsWorld>`
    //   * `pub particles: Option<&'a mut ParticleSystem>`
    // Backward-compatible: a consumer only naming `render_ctx` / `engine`
    // continues to compile because the new fields are pure additions.
}
