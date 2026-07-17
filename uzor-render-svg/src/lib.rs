//! `uzor-render-svg` — SVG serialization backend for uzor.
//!
//! Implements [`uzor::render::RenderContext`] via [`SvgRenderContext`], but
//! instead of rasterizing draw calls into a pixmap, it SERIALIZES them into
//! a standalone, portable SVG document string ([`SvgRenderContext::finish`])
//! — real vector paths, outlined text (via [`uzor::shaper::text_to_path`],
//! fidelity-first per `nemo/docs/uzor-engines/research_export_sota_2026.md`
//! §6's own verdict) plus an invisible companion `<text>` element so the
//! document stays searchable/selectable, and raster images embedded as
//! base64 PNG `<image>` elements.
//!
//! Incubating (`publish = false`), same tier as `uzor-export`/
//! `uzor-figures`/`uzor-typeset` — API surface not yet stable.
//!
//! # Design notes (report)
//!
//! - **Transforms are baked into every emitted coordinate**, not expressed
//!   as nested `<g transform="...">` scopes per `save`/`restore` — simpler
//!   (one flat element list, no scope-tracking `</g>` bookkeeping to keep
//!   balanced across arbitrary save/restore nesting) and more portable
//!   (plain numeric path/rect coordinates are understood by literally
//!   every SVG consumer, whereas relying on nested `transform` stacking
//!   correctly is a strictly harder property to guarantee as more state
//!   gets threaded through). The one documented exception is a
//!   ROTATED/SKEWED `<image>` element (`draw_image_rgba`) — raster content
//!   genuinely cannot be expressed as path `d` coordinates, so that one
//!   element carries its own `transform="matrix(...)"` attribute; every
//!   other draw call bakes fully into coordinates. Stroke width / dash
//!   arrays / radial-gradient radii are separate SVG attributes (not path
//!   geometry) and are scaled by [`crate::transform`]'s own approximate
//!   uniform `scale_factor()` so they still visually track `Painter::scale`
//!   under a baked transform.
//! - **Text** is painted as real outlined glyph paths (kept
//!   searchable/selectable via an accompanying `opacity="0"` `<text>`
//!   element, not embedded/subsetted live fonts) — matches the research
//!   doc's "fidelity-first" verdict for this backend's use case
//!   (report/chart export) over the alternative "live `<text>` + embedded
//!   WOFF2" strategy (trades outline fidelity for smaller files/native
//!   text selection — a plausible follow-up, not built this pass).
//! - **`BatchPainter`/`Effects`/`UiEffectHelpers`** are left at their own
//!   trait-default bodies — already-documented, non-silent fallbacks:
//!   `BatchPainter`'s defaults already loop over this backend's own
//!   `Painter` primitives (this task's own "correct beats clever, loops
//!   over the single-ops" instruction — literally free once `Painter` is
//!   real); `Effects`/`UiEffectHelpers` have no vector-SVG equivalent
//!   attempted this pass (shadow/blend-mode/backdrop-blur).
//! - **`GradientPainter`** IS a real implementation (`<linearGradient>`/
//!   `<radialGradient>` defs), not the trait's own flat-fill fallback.
//! - **`BackdropBlur`** (opt-in, not part of the `RenderContext` supertrait)
//!   is not implemented — a raster backdrop-sample blur has no vector-SVG
//!   equivalent.

mod context;
mod transform;
mod xml;

pub use context::SvgRenderContext;
