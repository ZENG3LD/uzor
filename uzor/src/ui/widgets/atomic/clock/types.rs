//! Clock widget type definitions.

use crate::render::RenderContext;
use crate::types::{Rect, WidgetState};

use super::settings::ClockSettings;

/// Selects the visual variant used by `draw_clock`.
pub enum ClockRenderKind {
    /// Narrow text strip with hover background — matches mlc toolbar clock.
    /// Fixed display width of 140 px; text right-aligned, monospace 13 px.
    Toolbar,
    /// Caller-supplied renderer — bypasses all built-in draw logic.
    Custom(
        Box<
            dyn Fn(
                &mut dyn RenderContext,
                Rect,
                WidgetState,
                &super::render::ClockView,
                &ClockSettings,
            ),
        >,
    ),
}
