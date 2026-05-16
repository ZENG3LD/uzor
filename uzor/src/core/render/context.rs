//! [`RenderContext`] — compound supertrait composition (uzor 2.0).
//!
//! `RenderContext` is now a thin supertrait that composes all required
//! capability traits. `&dyn RenderContext` works identically to the 1.x API —
//! all previously flat methods are reachable via supertrait dispatch.
//!
//! Opt-in capabilities ([`BackdropBlur`], [`ImagePainter`]) are declared by
//! backends independently and are NOT part of this supertrait.

use super::painter::Painter;
use super::text_renderer::TextRenderer;
use super::text_metrics::TextMetrics;
use super::masking::Masking;
use super::effects::Effects;
use super::shape_helpers::ShapeHelpers;
use super::batch_painter::BatchPainter;
use super::gradient::GradientPainter;
use super::ui_effects::UiEffectHelpers;

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
