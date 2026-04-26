//! Container rendering — bg + optional border + optional shadow.

use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::ContainerSettings;
use super::types::ContainerType;

pub struct ContainerView {
    pub kind: ContainerType,
    pub border: bool,
}

pub fn draw_container(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ContainerView,
    settings: &ContainerSettings,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    // Shadow first (Card only).
    if matches!(view.kind, ContainerType::Card) {
        let (sx, sy) = style.shadow_offset();
        ctx.set_fill_color_alpha(theme.shadow(), style.shadow_alpha());
        ctx.fill_rounded_rect(rect.x + sx, rect.y + sy, rect.width, rect.height, style.radius());
    }

    // Background.
    ctx.set_fill_color(theme.bg());
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());

    // Border.
    if view.border {
        ctx.set_stroke_color(theme.border());
        ctx.set_stroke_width(style.border_width());
        ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());
    }

    // Clip — renderer applies clip; caller draws children inside.
    if matches!(view.kind, ContainerType::Clip) {
        ctx.clip_rect(rect.x, rect.y, rect.width, rect.height);
    }
}
