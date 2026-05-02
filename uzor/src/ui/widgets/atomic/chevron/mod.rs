//! Chevron — general-purpose directional caret atomic.
//!
//! Subsumes the older `scroll_chevron`, dropdown-trigger triangles, submenu
//! markers, expand/collapse carets, panel-hide toggles, breadcrumb back arrows
//! and "this is a dropdown button" affordance dots. One atomic, five
//! orthogonal axes (use case, direction, visibility, placement, hit area)
//! plus a small `VisualKind` enum (Stroked / Filled / Glyph / Icon).
//!
//! # File layout
//! - `types.rs`   — `ChevronDirection`, `ChevronUseCase`, `VisibilityPolicy`,
//!                  `PlacementPolicy`, `HitAreaPolicy`, `ChevronVisualKind`,
//!                  `ChevronView`.
//! - `state.rs`   — interaction flags (mostly empty — chevron is stateless).
//! - `theme.rs`   — colours (normal/hover/disabled/bg-hover).
//! - `style.rs`   — sizes, stroke thickness, hover-bg radius.
//! - `settings.rs`— theme + style bundle.
//! - `render.rs`  — `draw_chevron(ctx, rect, view, settings)`.
//! - `input.rs`   — `register_*_chevron` for the three composition layers.

pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod types;

pub use input::*;
pub use render::*;
pub use settings::*;
pub use state::*;
pub use style::*;
pub use theme::*;
pub use types::*;
