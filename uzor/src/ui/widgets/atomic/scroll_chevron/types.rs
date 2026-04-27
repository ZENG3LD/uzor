//! Scroll chevron type definitions.

use crate::render::RenderContext;
use crate::types::{Rect, WidgetState};

use super::settings::ScrollChevronSettings;

/// Selects the visual variant used by `draw_scroll_chevron`.
pub enum ScrollChevronRenderKind {
    /// Default 16 × 16 px chevron (mlc toolbar_core.rs).
    Default,
    /// Caller-supplied renderer — bypasses all built-in draw logic.
    Custom(
        Box<
            dyn Fn(
                &mut dyn RenderContext,
                Rect,
                WidgetState,
                &super::render::ScrollChevronView,
                &ScrollChevronSettings,
            ),
        >,
    ),
}
