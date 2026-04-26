//! Scrollbar rendering — track + thumb.

use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::ScrollbarSettings;
use super::types::{ScrollbarOrientation, ScrollbarType};

pub struct ScrollbarView {
    pub kind: ScrollbarType,
    /// Scroll offset 0..1.
    pub offset: f64,
    /// Visible portion 0..1 (thumb size relative to track).
    pub visible_ratio: f64,
    pub hovered: bool,
    pub dragging: bool,
    /// Overlay opacity 0..1 (only used for `Overlay` variant).
    pub opacity: f64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ScrollbarResult {
    pub track_rect: Rect,
    pub thumb_rect: Rect,
}

pub fn draw_scrollbar(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ScrollbarView,
    settings: &ScrollbarSettings,
) -> ScrollbarResult {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let opacity = match view.kind {
        ScrollbarType::Standard { .. } => 1.0,
        ScrollbarType::Overlay  { .. } => view.opacity.clamp(0.0, 1.0),
    };
    if opacity <= 0.0 {
        return ScrollbarResult::default();
    }

    // Track
    let pad = style.track_padding();
    let track_rect = Rect::new(rect.x + pad, rect.y + pad, rect.width - pad * 2.0, rect.height - pad * 2.0);
    ctx.set_fill_color_alpha(theme.track(), opacity);
    ctx.fill_rounded_rect(track_rect.x, track_rect.y, track_rect.width, track_rect.height, style.thumb_radius());

    // Thumb geometry
    let (thumb_rect, _) = match view.kind.orientation() {
        ScrollbarOrientation::Vertical => {
            let track_len  = track_rect.height;
            let thumb_len  = (track_len * view.visible_ratio).max(style.thumb_min_length()).min(track_len);
            let max_off    = (track_len - thumb_len).max(0.0);
            let thumb_y    = track_rect.y + max_off * view.offset.clamp(0.0, 1.0);
            (Rect::new(track_rect.x, thumb_y, track_rect.width, thumb_len), thumb_len)
        }
        ScrollbarOrientation::Horizontal => {
            let track_len  = track_rect.width;
            let thumb_len  = (track_len * view.visible_ratio).max(style.thumb_min_length()).min(track_len);
            let max_off    = (track_len - thumb_len).max(0.0);
            let thumb_x    = track_rect.x + max_off * view.offset.clamp(0.0, 1.0);
            (Rect::new(thumb_x, track_rect.y, thumb_len, track_rect.height), thumb_len)
        }
    };

    let thumb_color = if view.dragging {
        theme.thumb_active()
    } else if view.hovered {
        theme.thumb_hover()
    } else {
        theme.thumb_normal()
    };
    ctx.set_fill_color_alpha(thumb_color, opacity);
    ctx.fill_rounded_rect(thumb_rect.x, thumb_rect.y, thumb_rect.width, thumb_rect.height, style.thumb_radius());

    ScrollbarResult { track_rect, thumb_rect }
}
