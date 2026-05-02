//! Chevron rendering — picks a colour from theme + draws via the
//! configured `ChevronVisualKind`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::Rect;

use super::settings::ChevronSettings;
use super::types::{ChevronDirection, ChevronView, ChevronVisualKind};

/// Draw the chevron into `rect` using `view` + `settings`.
///
/// Honours `view.should_render()` — bails out early if the chevron is
/// hidden by its visibility policy. Draws a hover background when the
/// chevron is hovered and not disabled / decorative.
pub fn draw_chevron(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ChevronView,
    settings: &ChevronSettings,
) {
    if !view.should_render() {
        return;
    }
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }

    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    // Colour resolution — disabled > pressed > active > hover > normal.
    let color = if view.disabled {
        theme.color_disabled()
    } else if view.pressed {
        theme.color_pressed()
    } else if view.active {
        theme.color_active()
    } else if view.hovered {
        theme.color_hover()
    } else {
        theme.color()
    };

    // Hover background — only for interactive use cases.
    let interactive = !matches!(
        view.use_case,
        super::types::ChevronUseCase::Affordance | super::types::ChevronUseCase::IconGlyph
    );
    if interactive && view.hovered && !view.disabled {
        ctx.set_fill_color(theme.bg_hover());
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.hover_bg_radius());
    }

    match view.visual_kind {
        ChevronVisualKind::Stroked  => draw_stroked(ctx, rect, view.direction, color, style),
        ChevronVisualKind::Filled   => draw_filled(ctx, rect, view.direction, color, style),
        ChevronVisualKind::Glyph    => draw_glyph(ctx, rect, view, color, style),
        ChevronVisualKind::Icon     => {
            // Caller-driven — atomic just reserves the rect; the host icon
            // system paints inside it. We do nothing here, so the rect is
            // already prepared (hover bg drawn above).
        }
    }
}

// ── Stroked V-shape (legacy scroll_chevron look) ─────────────────────────────

fn draw_stroked(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    direction: ChevronDirection,
    color:     &str,
    style:     &dyn super::style::ChevronStyle,
) {
    let inset = style.inset();
    let cx    = rect.center_x();
    let cy    = rect.center_y();
    let half  = (rect.width.min(rect.height) / 2.0 - inset).max(2.0);

    ctx.set_stroke_color(color);
    ctx.set_stroke_width(style.thickness());
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    match direction {
        ChevronDirection::Left => {
            ctx.move_to(cx + half * 0.5, cy - half);
            ctx.line_to(cx - half * 0.5, cy);
            ctx.line_to(cx + half * 0.5, cy + half);
        }
        ChevronDirection::Right => {
            ctx.move_to(cx - half * 0.5, cy - half);
            ctx.line_to(cx + half * 0.5, cy);
            ctx.line_to(cx - half * 0.5, cy + half);
        }
        ChevronDirection::Up => {
            ctx.move_to(cx - half, cy + half * 0.5);
            ctx.line_to(cx,        cy - half * 0.5);
            ctx.line_to(cx + half, cy + half * 0.5);
        }
        ChevronDirection::Down => {
            ctx.move_to(cx - half, cy - half * 0.5);
            ctx.line_to(cx,        cy + half * 0.5);
            ctx.line_to(cx + half, cy - half * 0.5);
        }
    }
    ctx.stroke();
}

// ── Filled triangle (split-button trigger / panel toggle look) ───────────────

fn draw_filled(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    direction: ChevronDirection,
    color:     &str,
    style:     &dyn super::style::ChevronStyle,
) {
    let s   = style.triangle_size();
    let cx  = rect.center_x();
    let cy  = rect.center_y();
    let hb  = s / 2.0; // half base
    let ah  = s / 2.0; // arrow half-height

    ctx.set_fill_color(color);
    ctx.begin_path();
    match direction {
        ChevronDirection::Down => {
            ctx.move_to(cx - hb, cy - ah);
            ctx.line_to(cx + hb, cy - ah);
            ctx.line_to(cx,      cy + ah);
        }
        ChevronDirection::Up => {
            ctx.move_to(cx - hb, cy + ah);
            ctx.line_to(cx + hb, cy + ah);
            ctx.line_to(cx,      cy - ah);
        }
        ChevronDirection::Left => {
            ctx.move_to(cx + ah, cy - hb);
            ctx.line_to(cx + ah, cy + hb);
            ctx.line_to(cx - ah, cy);
        }
        ChevronDirection::Right => {
            ctx.move_to(cx - ah, cy - hb);
            ctx.line_to(cx - ah, cy + hb);
            ctx.line_to(cx + ah, cy);
        }
    }
    ctx.close_path();
    ctx.fill();
}

// ── Unicode glyph ────────────────────────────────────────────────────────────

fn draw_glyph(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &ChevronView,
    color: &str,
    style: &dyn super::style::ChevronStyle,
) {
    let glyph = view.glyph_override.unwrap_or(match view.direction {
        ChevronDirection::Up    => "\u{25B2}",  // ▲
        ChevronDirection::Down  => "\u{25BC}",  // ▼
        ChevronDirection::Left  => "\u{25C0}",  // ◀
        ChevronDirection::Right => "\u{25B6}",  // ▶
    });
    ctx.set_fill_color(color);
    ctx.set_font(&format!("{}px sans-serif", style.glyph_size()));
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(glyph, rect.center_x(), rect.center_y());
}

// ── Measurement ──────────────────────────────────────────────────────────────

/// Natural visual size of a chevron given its style. Used by hosts that
/// reserve layout space for the atomic (e.g. toolbar overflow chevrons,
/// split-button trigger zones).
pub fn measure_chevron(settings: &ChevronSettings) -> (f64, f64) {
    let s = settings.style.size();
    (s, s)
}
