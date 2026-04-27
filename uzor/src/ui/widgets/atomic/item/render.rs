//! Item widget rendering — non-interactive label / icon / icon+text.
//!
//! Ported from `button/render.rs` `draw_toolbar_label` / `LabelView`.
//! Generalized to support icon-only and icon+text via `ItemRenderKind`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{IconId, Rect, WidgetState};

use super::settings::ItemSettings;
use super::types::ItemRenderKind;

/// Per-instance data for `draw_item`.
pub struct ItemView<'a> {
    /// Optional text label.
    pub label: Option<&'a str>,
    /// Optional icon identifier.
    pub icon: Option<&'a IconId>,
    /// Optional SVG string (used with `ItemRenderKind::Svg`).
    pub svg: Option<&'a str>,
}

/// Render an item widget, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`       — render context.
/// - `rect`      — bounding rect.
/// - `state`     — interaction state (always `Normal` — item has Sense::NONE).
/// - `view`      — per-frame data (label, icon, svg).
/// - `settings`  — visual configuration.
/// - `kind`      — which render variant to use.
/// - `draw_icon` — caller-supplied icon renderer closure.
///                 Signature: `(ctx, icon, icon_rect, color)`.
///                 Called for `Icon`, `TextIcon` variants.
/// - `draw_svg`  — caller-supplied SVG renderer closure.
///                 Signature: `(ctx, svg_str, rect, color)`.
///                 Called for the `Svg` variant.
pub fn draw_item<'a, FI, FS>(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    state:     WidgetState,
    view:      &ItemView<'a>,
    settings:  &ItemSettings,
    kind:      &ItemRenderKind<'a>,
    draw_icon: FI,
    draw_svg:  FS,
)
where
    FI: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
    FS: FnOnce(&mut dyn RenderContext, &str, Rect, &str),
{
    match kind {
        ItemRenderKind::Label => {
            draw_item_label(ctx, rect, view, settings);
            let _ = draw_icon;
            let _ = draw_svg;
        }
        ItemRenderKind::Icon => {
            draw_item_icon(ctx, rect, view, settings, draw_icon);
            let _ = draw_svg;
        }
        ItemRenderKind::TextIcon => {
            draw_item_text_icon(ctx, rect, view, settings, draw_icon);
            let _ = draw_svg;
        }
        ItemRenderKind::Svg => {
            draw_item_svg(ctx, rect, view, settings, draw_svg);
            let _ = draw_icon;
        }
        ItemRenderKind::Custom(f) => {
            f(ctx, rect, state, view, settings);
            let _ = draw_icon;
            let _ = draw_svg;
        }
    }
}

fn draw_item_label(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ItemView<'_>,
    settings: &ItemSettings,
) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    if let Some(text) = view.label {
        ctx.set_font(style.font());
        ctx.set_fill_color(theme.item_text());
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(text, rect.x + style.padding_x(), rect.center_y());
    }
}

fn draw_item_icon<F>(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    view:      &ItemView<'_>,
    settings:  &ItemSettings,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let theme     = settings.theme.as_ref();
    let style     = settings.style.as_ref();
    let icon_size = style.icon_size();

    if let Some(icon) = view.icon {
        let icon_rect = Rect::new(
            rect.center_x() - icon_size / 2.0,
            rect.center_y() - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_icon(ctx, icon, icon_rect, theme.item_text());
    }
}

fn draw_item_text_icon<F>(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    view:      &ItemView<'_>,
    settings:  &ItemSettings,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let theme     = settings.theme.as_ref();
    let style     = settings.style.as_ref();
    let icon_size = style.icon_size();
    let color     = theme.item_text();

    let mut text_x = rect.x + style.padding_x();

    if let Some(icon) = view.icon {
        let icon_rect = Rect::new(
            text_x,
            rect.center_y() - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_icon(ctx, icon, icon_rect, color);
        text_x += icon_size + style.icon_text_gap();
    }

    if let Some(text) = view.label {
        ctx.set_font(style.font());
        ctx.set_fill_color(color);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(text, text_x, rect.center_y());
    }
}

fn draw_item_svg<F>(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ItemView<'_>,
    settings: &ItemSettings,
    draw_svg: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &str, Rect, &str),
{
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();
    let icon_size = style.icon_size();

    if let Some(svg) = view.svg {
        let icon_rect = Rect::new(
            rect.center_x() - icon_size / 2.0,
            rect.center_y() - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_svg(ctx, svg, icon_rect, theme.item_text());
    }
}
