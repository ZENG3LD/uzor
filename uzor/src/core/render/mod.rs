//! Platform-agnostic rendering abstraction for uzor
//!
//! # uzor 2.0 — Capability Traits
//!
//! `RenderContext` is now a supertrait composition of focused capability traits.
//! All 1.x call sites using `&dyn RenderContext` continue to work unchanged.
//!
//! ## Required (all backends)
//! - [`Painter`] — path construction, fill/stroke, transforms, state
//! - [`TextRenderer`] — stateful text drawing
//! - [`TextMetrics`] — text measurement queries
//! - [`Masking`] — clip and mask layers
//! - [`Effects`] — shadow and blend mode
//! - [`ShapeHelpers`] — rect, rounded-rect helpers
//! - [`GradientPainter`] — linear and radial gradient fills
//! - [`UiEffectHelpers`] — hover/active/glass UI patterns
//!
//! ## Opt-in (declared by backends that support them)
//! - [`BackdropBlur`] — backdrop blur for FrostedGlass/LiquidGlass effects
//! - [`ImagePainter`] — image rendering

mod context;
mod painter;
mod text_renderer;
mod text_metrics;
mod masking;
mod effects;
mod shape_helpers;
mod gradient;
mod ui_effects;
mod backdrop_blur;
mod image_painter;
mod types;
mod helpers;
mod ops;
mod svg;
mod color;
mod region;

pub use crate::ui::assets::icons;

// Compound trait + ext
pub use context::{RenderContext, RenderContextExt};

// BlendMode re-exported at crate render root for backward compat
pub use effects::BlendMode;

// Capability traits — required
pub use painter::Painter;
pub use text_renderer::TextRenderer;
pub use text_metrics::TextMetrics;
pub use masking::Masking;
pub use effects::Effects;
pub use shape_helpers::ShapeHelpers;
pub use gradient::GradientPainter;
pub use ui_effects::UiEffectHelpers;

// Capability traits — opt-in
pub use backdrop_blur::BackdropBlur;
pub use image_painter::ImagePainter;

// Existing types (unchanged)
pub use types::{TextAlign, TextBaseline};
pub use helpers::{crisp, crisp_rect};
pub use ops::{RenderOp, RenderOps, execute_ops};
pub use svg::{draw_svg_icon, draw_svg_icon_rotated, draw_svg_multicolor};
pub use color::parse_color;
pub use region::{RenderRegion, RegionScheduleState, TickRate, UNCAPPED_FPS};
