//! Tooltip rendering — bg + border + text. Caller computes the rect
//! (using `tooltip_rect_from_anchor` helper) and the alpha (from `state`).

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::Rect;

use super::settings::TooltipSettings;
use super::types::{TooltipConfig, TooltipPosition};

/// Compute the tooltip's rect from the anchor + position + measured text width.
pub fn tooltip_rect_from_anchor(
    anchor: Rect,
    position: TooltipPosition,
    text_width: f64,
    style: &dyn TooltipStyle,
) -> Rect {
    let w = text_width + style.padding_x() * 2.0;
    let h = style.font_size() * 1.4 + style.padding_y() * 2.0;
    let gap = style.anchor_gap();
    match position {
        TooltipPosition::Above => Rect::new(
            anchor.x + (anchor.width - w) / 2.0,
            anchor.y - h - gap,
            w, h,
        ),
        TooltipPosition::Below => Rect::new(
            anchor.x + (anchor.width - w) / 2.0,
            anchor.y + anchor.height + gap,
            w, h,
        ),
        TooltipPosition::Left => Rect::new(
            anchor.x - w - gap,
            anchor.y + (anchor.height - h) / 2.0,
            w, h,
        ),
        TooltipPosition::Right => Rect::new(
            anchor.x + anchor.width + gap,
            anchor.y + (anchor.height - h) / 2.0,
            w, h,
        ),
    }
}

use super::style::TooltipStyle;

pub fn draw_tooltip(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    config: &TooltipConfig,
    alpha: f64,
    settings: &TooltipSettings,
) {
    if alpha <= 0.0 {
        return;
    }
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    ctx.set_fill_color_alpha(theme.bg(), alpha);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());

    ctx.set_stroke_color(theme.border());
    ctx.set_stroke_width(style.border_width());
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());

    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    ctx.set_fill_color_alpha(theme.text(), alpha);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(&config.text, rect.x + style.padding_x(), rect.y + rect.height / 2.0);
}
