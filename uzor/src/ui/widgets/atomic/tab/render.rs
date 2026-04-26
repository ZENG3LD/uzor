//! Tab rendering — bg + label/icon + optional close button.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::Rect;

use super::settings::TabSettings;
use super::types::TabConfig;

pub struct TabView<'a> {
    pub tab: &'a TabConfig,
    pub hovered: bool,
    pub pressed: bool,
    pub close_btn_hovered: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TabResult {
    /// Rect of the close button (if `closable`); zero rect otherwise.
    pub close_rect: Rect,
}

pub fn draw_tab(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &TabView<'_>,
    settings: &TabSettings,
) -> TabResult {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    // Background (active wins over hover).
    let bg = if view.tab.active {
        theme.bg_active()
    } else if view.hovered || view.pressed {
        theme.bg_hover()
    } else {
        theme.bg_normal()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());

    // Active accent bar (left edge).
    if view.tab.active {
        ctx.set_fill_color(theme.accent());
        ctx.fill_rect(rect.x, rect.y, style.accent_bar(), rect.height);
    }

    // Content layout: icon (optional) + label.
    let pad_x = style.padding_x();
    let mut text_x = rect.x + pad_x;

    if let Some(_icon) = view.tab.icon.as_ref() {
        // Icon rendering deferred to caller (no IconId in TabConfig — uses string name).
        // Reserve the space.
        text_x += style.icon_size() + style.gap();
    }

    // Label.
    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    ctx.set_fill_color(if view.tab.active { theme.text_active() } else { theme.text_normal() });
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(&view.tab.label, text_x, rect.y + rect.height / 2.0);

    // Close button (right-aligned).
    let mut close_rect = Rect::default();
    if view.tab.closable {
        let s = style.close_btn_size();
        let cx = rect.x + rect.width - pad_x - s;
        let cy = rect.y + (rect.height - s) / 2.0;
        close_rect = Rect::new(cx, cy, s, s);
        let close_color = if view.close_btn_hovered { theme.close_hover() } else { theme.close_normal() };
        ctx.set_stroke_color(close_color);
        ctx.set_stroke_width(1.5);
        // X mark
        ctx.set_fill_color(close_color);
        ctx.fill_rect(cx + s * 0.45, cy + s * 0.15, 1.5, s * 0.7);
        ctx.fill_rect(cx + s * 0.15, cy + s * 0.45, s * 0.7, 1.5);
    }

    TabResult { close_rect }
}
