//! Close button type definitions.

use crate::render::RenderContext;
use crate::types::{Rect, WidgetState};

use super::settings::CloseButtonSettings;

/// Selects the visual variant used by `draw_close_button`.
pub enum CloseButtonRenderKind {
    /// Default 18 × 18 px close button (chart_settings / indicator_settings).
    Default,
    /// Larger 28 × 28 px close button with hover bg fill (profile_manager).
    Large,
    /// Caller-supplied renderer — bypasses all built-in draw logic.
    Custom(
        Box<
            dyn Fn(
                &mut dyn RenderContext,
                Rect,
                WidgetState,
                &super::render::CloseButtonView,
                &CloseButtonSettings,
            ),
        >,
    ),
}
