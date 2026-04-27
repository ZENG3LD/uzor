//! Item widget type definitions.

use crate::render::RenderContext;
use crate::types::{Rect, WidgetState};

use super::settings::ItemSettings;

/// Selects the visual variant used by `draw_item`.
///
/// The item widget is non-clickable (Sense::NONE). It is used for labels,
/// icons, and icon+text combinations in lists, menus, and toolbars.
pub enum ItemRenderKind<'a> {
    /// Text-only label.
    Label,
    /// Icon-only display.
    /// The caller provides the icon via `ItemView::icon`.
    Icon,
    /// Icon left + text right.
    TextIcon,
    /// Caller-supplied SVG string rendered as a glyph.
    /// The caller provides the SVG data via `ItemView::svg`.
    Svg,
    /// Caller-supplied renderer — bypasses all built-in draw logic.
    Custom(
        Box<
            dyn Fn(
                &mut dyn RenderContext,
                Rect,
                WidgetState,
                &super::render::ItemView<'a>,
                &ItemSettings,
            ),
        >,
    ),
}
