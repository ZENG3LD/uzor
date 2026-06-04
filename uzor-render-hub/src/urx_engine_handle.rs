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

/// Fat handle handed to the closure passed to `RenderHub::with_urx_engine`.
///
/// Lifetime `'a` binds to the borrow of the `WindowRenderState` taken
/// for the duration of the callback. All fields are mutable borrows —
/// the consumer may freely mutate any subset.
pub struct UrxEngineHandle<'a> {
    /// Standard 2D scene (Canvas2D shape — fill_rect/stroke/text/...).
    /// Same `RenderContext` trait the legacy channel uses, so a
    /// consumer that ignores the rest of this handle still draws
    /// pixels with one line.
    pub render_ctx: &'a mut dyn RenderContext,

    /// Frame dimensions in physical pixels (post-DPI scaling).
    pub width: u32,
    pub height: u32,
    /// Device pixel ratio in effect.
    pub dpr: f64,
    /// Monotonic frame counter for this window.
    pub frame_idx: u64,
    // ── Stage 3+ fields (currently always `None`) ─────────────────────────
    //
    // The plan deliberately keeps these absent from Stage 1 — adding the
    // field with `pub engine: Option<&'a mut UrxEngine>` etc. brings in
    // the urx-engine API surface before the slot itself exists in
    // WindowRenderState. Stage 3 will:
    //   * add UrxEngine slot in WindowRenderState
    //   * extend this handle with `pub engine: &'a mut UrxEngine`
    //   * extend it with `pub r3d: Option<&'a mut Renderer3D>` etc.
    // Backward-compatible: a consumer only naming `render_ctx` continues
    // to compile because the new fields are pure additions.
}
