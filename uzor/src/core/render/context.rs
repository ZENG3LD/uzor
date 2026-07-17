//! [`RenderContext`] — compound supertrait composition (uzor 2.0).
//!
//! `RenderContext` is now a thin supertrait that composes all required
//! capability traits. `&dyn RenderContext` works identically to the 1.x API —
//! all previously flat methods are reachable via supertrait dispatch.
//!
//! Opt-in capabilities ([`BackdropBlur`], [`ImagePainter`]) are declared by
//! backends independently and are NOT part of this supertrait. [`ImagePainter`]
//! is additionally reachable from `&mut dyn RenderContext` via the
//! [`RenderContext::image_painter`] capability-query accessor — the same
//! "declare it, callers check explicitly" convention `BackdropBlur` uses
//! at the concrete-type level.

use super::painter::Painter;
use super::text_renderer::TextRenderer;
use super::text_metrics::TextMetrics;
use super::masking::Masking;
use super::effects::Effects;
use super::shape_helpers::ShapeHelpers;
use super::batch_painter::BatchPainter;
use super::gradient::GradientPainter;
use super::ui_effects::UiEffectHelpers;
use super::image_painter::ImagePainter;
use super::offscreen::{OffscreenTarget, OffscreenTargetDesc, OffscreenTargetId};
use crate::core::types::Rect;

/// Platform-agnostic rendering context — the full drawing surface.
///
/// Composes all required capability traits. Use `&dyn RenderContext` exactly
/// as in uzor 1.x — all methods are reachable via supertrait dispatch.
///
/// Backends that support blur or image rendering declare those separately:
/// - [`BackdropBlur`](super::BackdropBlur) — opt-in
/// - [`ImagePainter`](super::ImagePainter) — opt-in
pub trait RenderContext:
    Painter
    + TextRenderer
    + TextMetrics
    + Masking
    + Effects
    + ShapeHelpers
    + BatchPainter
    + GradientPainter
    + UiEffectHelpers
{
    /// Device pixel ratio for crisp rendering.
    fn dpr(&self) -> f64;

    /// Capability query — opt-in image compositing
    /// ([`ImagePainter`]). Backends that can composite raster images
    /// override this to return `Some(self)`; callers check the result
    /// and fall back (e.g. a placeholder) when it's `None`, the same
    /// "declare capability, caller checks explicitly" convention this
    /// module's own doc comment already documents for `BackdropBlur`/
    /// `ImagePainter` at the `RenderContext` supertrait level.
    fn image_painter(&mut self) -> Option<&mut dyn ImagePainter> {
        None
    }

    /// Capability query — walker checks ONCE per backend instance
    /// (not per node/frame) before attempting `push_offscreen_target`.
    fn supports_offscreen_targets(&self) -> bool {
        false
    }

    /// Begin painting into a fresh (or reused, see `resize_offscreen_target`)
    /// offscreen surface of `desc` dimensions. Every subsequent draw call
    /// on `self` targets the offscreen surface until the matching
    /// `pop_offscreen_target` — mirrors the existing `save`/`restore`
    /// stack discipline (`Painter::save`/`restore`), NOT a new stack of
    /// its own; backends implement it by swapping their internal
    /// "current surface" pointer, same pattern as `save`/`restore`
    /// swapping the transform/clip stack.
    ///
    /// Returns `None` when unsupported (default) — caller MUST NOT have
    /// emitted any draw calls it can't unwind; contract: call this
    /// BEFORE any drawing for the subtree, check the result, and only
    /// proceed with the offscreen path on `Some`.
    fn push_offscreen_target(&mut self, _desc: OffscreenTargetDesc) -> OffscreenTarget {
        None
    }

    /// End painting into the offscreen surface, restoring `self` to
    /// whatever surface was active before the matching `push`. No-op
    /// (and safe to call) when offscreen targets are unsupported —
    /// callers only reach this after a successful `push`, so the
    /// default body is unreachable in practice but kept total (no panic).
    fn pop_offscreen_target(&mut self) {}

    /// Composite a previously-populated target as a single quad into
    /// the CURRENTLY ACTIVE surface (whatever `self` is drawing into
    /// right now — screen or another offscreen target, for nested
    /// boundaries). `dst_rect` is in the current surface's local
    /// coordinate space (same convention as every other `Painter` draw
    /// call — caller has already `translate`d). Returns `false` when
    /// `id` is unknown to this backend (freed, wrong backend instance,
    /// or unsupported) — caller falls back to inline re-paint.
    fn draw_cached_target(&mut self, _id: OffscreenTargetId, _dst_rect: Rect) -> bool {
        false
    }

    /// Resize an existing target in place (viewport/DPI change).
    /// Returns `false` if `id` is unknown — caller should `free` +
    /// `push` a new one instead. Default no-op.
    fn resize_offscreen_target(&mut self, _id: OffscreenTargetId, _desc: OffscreenTargetDesc) -> bool {
        false
    }

    /// Release backend-side storage for `id`. Called when the kernel's
    /// repaint-boundary bookkeeping entry is dropped (node despawned,
    /// boundary behavior removed, or the store evicts under its memory
    /// cap). Default no-op (nothing was allocated).
    fn free_offscreen_target(&mut self, _id: OffscreenTargetId) {}
}

/// Extension trait for platform-specific blur features.
///
/// Provides type-safe blur image management for RenderContext implementations.
/// Kept as-is from 1.x for backend setup compatibility (revisit in 3.0).
pub trait RenderContextExt: RenderContext {
    type BlurImage: Clone;
    fn set_blur_image(&mut self, _image: Option<Self::BlurImage>, _width: u32, _height: u32) {}
    fn set_use_convex_glass_buttons(&mut self, _use_convex: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal `RenderContext` impl that does NOT override `image_painter` —
    // same boilerplate-minimal `MockContext` pattern already used throughout
    // this crate's widget test modules (e.g. `ui::themes::macos::widgets::
    // switch_toggle::tests::MockContext`), just renamed here to make the
    // "no image capability" intent explicit at the call site.
    struct NoImageContext;

    impl crate::render::Painter for NoImageContext {
        fn save(&mut self) {}
        fn restore(&mut self) {}
        fn translate(&mut self, _x: f64, _y: f64) {}
        fn rotate(&mut self, _angle: f64) {}
        fn scale(&mut self, _x: f64, _y: f64) {}
        fn set_fill_color(&mut self, _color: &str) {}
        fn set_global_alpha(&mut self, _alpha: f64) {}
        fn set_stroke_color(&mut self, _color: &str) {}
        fn set_stroke_width(&mut self, _width: f64) {}
        fn set_line_dash(&mut self, _pattern: &[f64]) {}
        fn set_line_cap(&mut self, _cap: &str) {}
        fn set_line_join(&mut self, _join: &str) {}
        fn begin_path(&mut self) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn close_path(&mut self) {}
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn arc(&mut self, _cx: f64, _cy: f64, _r: f64, _s: f64, _e: f64) {}
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _s: f64, _e: f64) {}
        fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
        fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}
        fn stroke(&mut self) {}
        fn fill(&mut self) {}
    }
    impl crate::render::TextRenderer for NoImageContext {
        fn set_font(&mut self, _font: &str) {}
        fn set_text_align(&mut self, _align: crate::render::TextAlign) {}
        fn set_text_baseline(&mut self, _baseline: crate::render::TextBaseline) {}
        fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) {}
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
    }
    impl crate::render::TextMetrics for NoImageContext {
        fn measure_text(&self, _text: &str) -> f64 {
            0.0
        }
        fn text_bounds(&self, _text: &str, _font: &str) -> crate::render::TextBounds {
            crate::render::TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 }
        }
    }
    impl crate::render::Masking for NoImageContext {
        fn clip(&mut self) {}
    }
    impl crate::render::Effects for NoImageContext {}
    impl crate::render::ShapeHelpers for NoImageContext {
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    }
    impl crate::render::GradientPainter for NoImageContext {}
    impl crate::render::UiEffectHelpers for NoImageContext {}
    impl crate::render::BatchPainter for NoImageContext {}
    impl RenderContext for NoImageContext {
        fn dpr(&self) -> f64 {
            1.0
        }
    }

    #[test]
    fn image_painter_default_is_none_for_a_backend_that_never_overrides_it() {
        let mut ctx = NoImageContext;
        let dyn_ctx: &mut dyn RenderContext = &mut ctx;
        assert!(dyn_ctx.image_painter().is_none());
    }
}
