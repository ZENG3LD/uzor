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
