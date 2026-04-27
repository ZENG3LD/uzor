//! Tooltip rendering — bg + border + text. Caller computes the rect
//! (using `tooltip_rect_from_anchor` helper) and the alpha (from `state`).
//!
//! Three render entry points:
//! - `draw_tooltip`           — generic uzor tooltip (back-compat)
//! - `draw_chrome_tooltip`    — matches mlc `chrome.rs:render_tooltip_themed`
//! - `draw_crosshair_tooltip` — matches mlc `overlays.rs:draw_tooltip`

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::Rect;

use super::settings::TooltipSettings;
use super::style::TooltipStyle;
use super::types::{TooltipConfig, TooltipPosition};

// ─── rect helpers ────────────────────────────────────────────────────────────

/// Compute the tooltip's rect from the anchor + position + measured text width.
///
/// For single-line tooltips. Height = `font_size + padding_y * 2`.
pub fn tooltip_rect_from_anchor(
    anchor: Rect,
    position: TooltipPosition,
    text_width: f64,
    style: &dyn TooltipStyle,
) -> Rect {
    let w = text_width + style.padding_x() * 2.0;
    let h = style.font_size() + style.padding_y() * 2.0;
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

/// Compute the tooltip rect for a multi-line crosshair tooltip.
///
/// - Measures each line; width = max(measured, `style.min_content_width()`) + padding*2.
/// - Height = `line_count * font_size * line_height_factor + padding_y * 2`.
/// - Position clamped to `container` bounds — no flip.
///
/// `cursor` is the screen-space crosshair position.
/// `container` is the area the tooltip must stay inside (chart rect or screen rect).
pub fn tooltip_multiline_rect(
    cursor: (f64, f64),
    lines: &[&str],
    style: &dyn TooltipStyle,
    ctx: &dyn RenderContext,
    container: Rect,
) -> Rect {
    let font_size = style.font_size();
    let line_height = font_size * style.line_height_factor();
    let pad_x = style.padding_x();
    let pad_y = style.padding_y();
    let gap = style.anchor_gap();

    let mut max_w = style.min_content_width();
    for line in lines {
        let w = ctx.measure_text(line);
        if w > max_w {
            max_w = w;
        }
    }
    let tw = max_w + pad_x * 2.0;
    let th = lines.len() as f64 * line_height + pad_y * 2.0;

    let raw_x = cursor.0 + gap;
    let raw_y = cursor.1 + gap;

    let clamped_x = raw_x.clamp(container.x, (container.x + container.width - tw).max(container.x));
    let clamped_y = raw_y.clamp(container.y, (container.y + container.height - th).max(container.y));

    Rect::new(clamped_x, clamped_y, tw, th)
}

/// Compute position for a Chrome/Toolbar single-line tooltip with auto-flip.
///
/// Tooltip is placed `anchor_gap` px below the cursor by default;
/// flips above when near the bottom edge, and flips left when near the right edge.
/// Falls back to screen edge pin if the flipped position also overflows.
///
/// `cursor` — pointer position in screen coords.
/// `tooltip_size` — (width, height) of the rendered box.
/// `screen` — full screen rect (or window bounds).
/// `anchor_gap` — vertical offset below cursor.
fn chrome_tooltip_position(
    cursor: (f64, f64),
    tooltip_size: (f64, f64),
    screen: Rect,
    anchor_gap: f64,
) -> (f64, f64) {
    let (tw, th) = tooltip_size;
    let mut x = cursor.0;
    let mut y = cursor.1 + anchor_gap;

    // Right overflow → flip left of cursor.
    if x + tw > screen.x + screen.width {
        x = cursor.0 - tw;
        if x < screen.x {
            x = screen.x + screen.width - tw;
        }
    }
    if x < screen.x { x = screen.x; }

    // Bottom overflow → flip above cursor.
    if y + th > screen.y + screen.height {
        y = cursor.1 - th - anchor_gap;
        if y < screen.y {
            y = screen.y + screen.height - th;
        }
    }
    if y < screen.y { y = screen.y; }

    (x, y)
}

// ─── render functions ─────────────────────────────────────────────────────────

/// Generic uzor tooltip — kept for back-compat.
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

    if style.border_width() > 0.0 {
        ctx.set_stroke_color(theme.border());
        ctx.set_stroke_width(style.border_width());
        ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());
    }

    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    ctx.set_fill_color_alpha(theme.text(), alpha);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(&config.text, rect.x + style.padding_x(), rect.y + rect.height / 2.0);
}

/// Render a Chrome/Toolbar single-line tooltip matching mlc `render_tooltip_themed`.
///
/// - Measures text, derives box size.
/// - Places `anchor_gap` px below `cursor`; auto-flips at screen edges.
/// - Draws 1-px drop shadow, rounded bg, text. No border stroke.
/// - `alpha` should come from `TooltipState::fade_in_progress` (0.0 → 1.0).
///
/// `screen` is the window bounds used for flip detection.
pub fn draw_chrome_tooltip(
    ctx: &mut dyn RenderContext,
    cursor: (f64, f64),
    text: &str,
    alpha: f64,
    settings: &TooltipSettings,
    screen: Rect,
) {
    if alpha <= 0.0 {
        return;
    }
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    let text_w = ctx.measure_text(text);
    let pad_x = style.padding_x();
    let pad_y = style.padding_y();
    let tw = text_w + pad_x * 2.0;
    // mlc hardcodes height as `font_size + pad*2` = 12 + 12 = 24 px.
    let th = style.font_size() + pad_y * 2.0;

    let (tx, ty) = chrome_tooltip_position(cursor, (tw, th), screen, style.anchor_gap());

    ctx.save();
    ctx.set_global_alpha(alpha);

    if style.has_shadow() {
        ctx.set_fill_color(theme.shadow());
        ctx.fill_rounded_rect(tx + 1.0, ty + 1.0, tw, th, style.radius());
    }

    ctx.set_fill_color(theme.bg());
    ctx.fill_rounded_rect(tx, ty, tw, th, style.radius());

    ctx.set_fill_color(theme.text());
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(text, tx + pad_x, ty + th / 2.0);

    ctx.restore();
}

/// Render a multi-line OHLC crosshair tooltip matching mlc `overlays.rs:draw_tooltip`.
///
/// - Lines from `config.resolved_lines()`.
/// - Box size: max line width + padding * 2; height: lines * line_height + padding * 2.
/// - Position clamped to `container` bounds — no flip.
/// - No alpha/fade — always renders at full opacity (mlc crosshair has no animation).
///
/// `cursor` — crosshair screen position (typically `crosshair.x + offset, crosshair.y + offset`).
/// `container` — the chart rect used for clamping.
pub fn draw_crosshair_tooltip(
    ctx: &mut dyn RenderContext,
    cursor: (f64, f64),
    config: &TooltipConfig,
    settings: &TooltipSettings,
    container: Rect,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let lines = config.resolved_lines();
    if lines.is_empty() {
        return;
    }

    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    let rect = tooltip_multiline_rect(cursor, &lines, style, ctx, container);

    let pad_x = style.padding_x();
    let pad_y = style.padding_y();
    let font_size = style.font_size();
    let line_height = font_size * style.line_height_factor();
    let radius = style.radius();

    ctx.set_fill_color(theme.bg());
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    if style.border_width() > 0.0 {
        ctx.set_stroke_color(theme.border());
        ctx.set_stroke_width(style.border_width());
        ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);
    }

    ctx.set_fill_color(theme.text());
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);

    let mut text_y = rect.y + pad_y + line_height * 0.5;
    for line in &lines {
        ctx.fill_text(line, rect.x + pad_x, text_y);
        text_y += line_height;
    }
}
