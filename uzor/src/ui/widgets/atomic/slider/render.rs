//! Slider rendering — ported from `mlc/chart/src/ui/widgets/slider.rs`.
//!
//! # Variants
//!
//! | Fn | Variant | Layout |
//! |----|---------|--------|
//! | `draw_single_slider` | 1.1 label+input, 1.2 editing, 1.3 no-input | `[Label] ──●── [Input]` |
//! | `draw_dual_slider`   | 1.4 dual handles | `[MinInput] ──●━━●── [MaxInput]` |
//! | `draw_line_width_slider` | 1.5 toolbar style | `──●── 3.5px` |
//!
//! All three functions take the **full row rect** and perform the
//! label / track / input geometry internally — callers do not pre-slice.
//!
//! The inline value box is drawn via `text_input::render::draw_input` for
//! proper border-focus + text-selection rendering (variant 1.2).

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetState};

use super::settings::SliderSettings;
use super::types::{
    DualSliderHandle, DualSliderResult, DualSliderView, LineWidthSliderResult,
    LineWidthSliderView, SingleSliderResult, SingleSliderView, SliderTrackInfo, SliderType,
};

use crate::ui::widgets::atomic::text_input::{
    render::{draw_input, InputView},
    settings::TextInputSettings,
    state::TextFieldConfig,
    types::InputType,
};

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn value_to_x(value: f64, min: f64, max: f64, track_x: f64, track_width: f64) -> f64 {
    if max <= min {
        return track_x;
    }
    let t = ((value - min) / (max - min)).clamp(0.0, 1.0);
    track_x + t * track_width
}

/// Build a minimal `TextInputSettings` wired to the slider's style/theme tokens
/// so the inline input box looks coherent with the rest of the slider.
fn make_input_settings(
    style: &dyn super::style::SliderStyle,
    theme: &dyn super::theme::SliderTheme,
) -> TextInputSettings {
    use crate::ui::widgets::atomic::text_input::{
        style::TextInputStyle, theme::TextInputTheme,
    };

    struct SliderInputTheme {
        bg: [u8; 4],
        border_n: [u8; 4],
        border_f: [u8; 4],
        text: [u8; 4],
    }
    impl TextInputTheme for SliderInputTheme {
        fn bg_normal(&self)        -> [u8; 4] { self.bg }
        fn bg_disabled(&self)      -> [u8; 4] { [35, 35, 35, 255] }
        fn border_normal(&self)    -> [u8; 4] { self.border_n }
        fn border_hover(&self)     -> [u8; 4] { self.border_f }
        fn border_focused(&self)   -> [u8; 4] { self.border_f }
        fn text_normal(&self)      -> [u8; 4] { self.text }
        fn text_disabled(&self)    -> [u8; 4] { [120, 123, 134, 255] }
        fn placeholder(&self)      -> [u8; 4] { [120, 123, 134, 255] }
        fn selection(&self)        -> [u8; 4] { [33, 150, 243, 128] }
        fn cursor(&self)           -> [u8; 4] { self.text }
    }

    struct SliderInputStyle {
        font_size: f64,
        padding: f64,
        radius: f64,
        border_n: f64,
        border_f: f64,
    }
    impl TextInputStyle for SliderInputStyle {
        fn height(&self)                       -> f64 { 22.0 }
        fn padding(&self)                      -> f64 { self.padding }
        fn radius(&self)                       -> f64 { self.radius }
        fn border_width_normal(&self)          -> f64 { self.border_n }
        fn border_width_focused(&self)         -> f64 { self.border_f }
        fn font_size(&self)                    -> f64 { self.font_size }
        fn cursor_width(&self)                 -> f64 { 1.0 }
        fn cursor_margin(&self)                -> f64 { 2.0 }
        fn cursor_blink_half_period_ms(&self)  -> u64 { 500 }
    }

    fn hex_to_rgba(hex: &str) -> [u8; 4] {
        let s = hex.trim_start_matches('#');
        if s.len() == 6 {
            let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(255);
            [r, g, b, 255]
        } else if s.len() == 8 {
            let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(255);
            let a = u8::from_str_radix(&s[6..8], 16).unwrap_or(255);
            [r, g, b, a]
        } else {
            [255, 255, 255, 255]
        }
    }

    TextInputSettings {
        theme: Box::new(SliderInputTheme {
            bg:       hex_to_rgba(theme.input_bg()),
            border_n: hex_to_rgba(theme.input_border_normal()),
            border_f: hex_to_rgba(theme.input_border_focused()),
            text:     hex_to_rgba(theme.text_normal()),
        }),
        style: Box::new(SliderInputStyle {
            font_size: style.font_size(),
            padding:   style.input_padding(),
            radius:    style.input_radius(),
            border_n:  style.input_border_width_normal(),
            border_f:  style.input_border_width_focused(),
        }),
        config: TextFieldConfig::text(),
    }
}

/// Draw the handle circle (with optional hover halo).
fn draw_handle(
    ctx: &mut dyn RenderContext,
    cx: f64,
    cy: f64,
    hovered: bool,
    disabled: bool,
    settings: &SliderSettings,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();
    let r = style.handle_radius();

    if hovered {
        let hr = r + style.handle_hover_ring();
        ctx.set_fill_color(theme.accent());
        ctx.set_global_alpha(0.25);
        ctx.fill_rounded_rect(cx - hr, cy - hr, hr * 2.0, hr * 2.0, hr);
        ctx.set_global_alpha(1.0);
    }

    // Body
    ctx.set_fill_color(if disabled { theme.text_disabled() } else { theme.text_normal() });
    ctx.fill_rounded_rect(cx - r, cy - r, r * 2.0, r * 2.0, r);

    // Border
    ctx.set_stroke_color(theme.handle_border());
    ctx.set_stroke_width(style.handle_border_width());
    ctx.stroke_rounded_rect(cx - r, cy - r, r * 2.0, r * 2.0, r);
}

/// Draw the track background + filled portion for a single handle.
/// Returns `handle_x`.
fn draw_single_track(
    ctx: &mut dyn RenderContext,
    track_x: f64,
    track_y: f64,
    track_w: f64,
    track_h: f64,
    radius: f64,
    value: f64,
    min: f64,
    max: f64,
    settings: &SliderSettings,
) -> f64 {
    let theme = settings.theme.as_ref();

    // Empty track
    ctx.set_fill_color(theme.track_empty());
    ctx.fill_rounded_rect(track_x, track_y, track_w, track_h, radius);

    let handle_x = value_to_x(value, min, max, track_x, track_w);

    // Filled left portion
    let fill_w = (handle_x - track_x).max(0.0);
    if fill_w > 0.0 {
        ctx.set_fill_color(theme.accent());
        ctx.fill_rounded_rect(track_x, track_y, fill_w, track_h, radius);
    }

    handle_x
}

/// Draw track for dual handles — filled portion is *between* handles.
/// Returns `(x_min, x_max)`.
fn draw_dual_track(
    ctx: &mut dyn RenderContext,
    track_x: f64,
    track_y: f64,
    track_w: f64,
    track_h: f64,
    radius: f64,
    min_value: f64,
    max_value: f64,
    min: f64,
    max: f64,
    settings: &SliderSettings,
) -> (f64, f64) {
    let theme = settings.theme.as_ref();

    ctx.set_fill_color(theme.track_empty());
    ctx.fill_rounded_rect(track_x, track_y, track_w, track_h, radius);

    let x_min = value_to_x(min_value, min, max, track_x, track_w);
    let x_max = value_to_x(max_value, min, max, track_x, track_w);
    let fill_w = (x_max - x_min).max(0.0);
    if fill_w > 0.0 {
        ctx.set_fill_color(theme.accent());
        ctx.fill_rounded_rect(x_min, track_y, fill_w, track_h, radius);
    }

    (x_min, x_max)
}

// ─── Variant 1.1 / 1.2 / 1.3 ─────────────────────────────────────────────────

/// Render a single-handle slider row.
///
/// - `view.label = Some(...)` → label drawn left (variants 1.1 / 1.2 / 1.3).
/// - `view.config.show_input = true` + `view.editing = None` → variant 1.1.
/// - `view.config.show_input = true` + `view.editing = Some(...)` → variant 1.2.
/// - `view.config.show_input = false` → variant 1.3.
///
/// `row_rect` is the **full** row rect (label + track + input).
pub fn draw_single_slider(
    ctx: &mut dyn RenderContext,
    row_rect: Rect,
    view: &SingleSliderView<'_>,
    settings: &SliderSettings,
) -> SingleSliderResult {
    let style  = settings.style.as_ref();
    let theme  = settings.theme.as_ref();
    let config = view.config;

    let center_y = row_rect.center_y();
    let track_h  = style.track_height();
    let track_y  = center_y - track_h / 2.0;
    let radius   = style.track_radius();

    ctx.set_font(&format!("{}px sans-serif", style.font_size()));

    // ── Label ────────────────────────────────────────────────────────────────
    let label_section_w = if let Some(label) = view.label {
        if label.is_empty() {
            0.0
        } else {
            ctx.set_fill_color(theme.text_normal());
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(label, row_rect.x, center_y);
            ctx.measure_text(label) + style.label_spacing()
        }
    } else {
        0.0
    };

    // ── Input section geometry ───────────────────────────────────────────────
    let input_section_w = if config.show_input {
        config.input_width + style.track_input_spacing()
    } else {
        0.0
    };

    // ── Track ────────────────────────────────────────────────────────────────
    let track_x = row_rect.x + label_section_w;
    let track_w = (row_rect.width - label_section_w - input_section_w).max(0.0);
    let track_rect = Rect::new(track_x, track_y, track_w, track_h);

    let handle_x = draw_single_track(
        ctx, track_x, track_y, track_w, track_h, radius,
        view.value, config.min, config.max,
        settings,
    );

    // ── Handle ───────────────────────────────────────────────────────────────
    let r = style.handle_radius();
    draw_handle(ctx, handle_x, center_y, view.hovered, view.disabled, settings);
    let handle_rect = Rect::new(handle_x - r, center_y - r, r * 2.0, r * 2.0);

    // ── Inline value input (variants 1.1 / 1.2) ──────────────────────────────
    let input_rect = if config.show_input {
        let ix = track_x + track_w + style.track_input_spacing();
        let iy = center_y - config.input_height / 2.0;
        let irect = Rect::new(ix, iy, config.input_width, config.input_height);

        let (text, cursor, selection, focused) = match &view.editing {
            Some(ed) => (
                ed.text,
                ed.cursor,
                ed.selection_start.map(|s| (s, ed.cursor)),
                true,
            ),
            None => (
                // We can't return the formatted string from here without
                // allocating; caller typically passes the formatted value.
                // Render an empty box so the caller can paint text on top,
                // OR pass an editing view with the static text pre-formatted.
                "",
                0,
                None,
                false,
            ),
        };

        // Build a display text buffer for the non-editing case.
        let formatted;
        let display_text = if text.is_empty() && !focused {
            formatted = config.format_value(view.value);
            formatted.as_str()
        } else {
            text
        };

        let iv = InputView {
            text:       display_text,
            placeholder: "",
            cursor,
            selection,
            focused,
            disabled: view.disabled,
            input_type: InputType::Number,
        };

        let input_settings = make_input_settings(style, theme);
        draw_input(ctx, irect, WidgetState::Normal, &iv, &input_settings);

        Some(irect)
    } else {
        None
    };

    // ── Track info ────────────────────────────────────────────────────────────
    let track_info = SliderTrackInfo {
        track_x,
        track_width: track_w,
        track_y: center_y - r,
        track_height: r * 2.0,
        min_val: config.min,
        max_val: config.max,
    };

    SingleSliderResult {
        full_rect: row_rect,
        track_rect,
        handle_rect,
        input_rect,
        track_info,
    }
}

// ─── Variant 1.4 ─────────────────────────────────────────────────────────────

/// Render a dual-handle slider row.
///
/// Min input is placed **left** of the track; max input is placed **right**.
/// This differs from single-slider where the input is always on the right.
///
/// `row_rect` is the full row rect.
pub fn draw_dual_slider(
    ctx: &mut dyn RenderContext,
    row_rect: Rect,
    view: &DualSliderView<'_>,
    settings: &SliderSettings,
) -> DualSliderResult {
    let style  = settings.style.as_ref();
    let theme  = settings.theme.as_ref();
    let config = view.config;

    let center_y = row_rect.center_y();
    let track_h  = style.track_height();
    let track_y  = center_y - track_h / 2.0;
    let radius   = style.track_radius();

    ctx.set_font(&format!("{}px sans-serif", style.font_size()));

    // ── Label ────────────────────────────────────────────────────────────────
    let label_section_w = if let Some(label) = view.label {
        if label.is_empty() {
            0.0
        } else {
            ctx.set_fill_color(theme.text_normal());
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(label, row_rect.x, center_y);
            ctx.measure_text(label) + style.label_spacing()
        }
    } else {
        0.0
    };

    // ── Dual-input geometry ──────────────────────────────────────────────────
    // Both inputs take from the total width; track fills what's left.
    // MinInput left, MaxInput right (symmetrical around the track).
    let inputs_section_w = config.input_width * 2.0 + style.track_input_spacing() * 2.0;
    let label_x = row_rect.x + label_section_w;
    let track_w = (row_rect.width - label_section_w - inputs_section_w).max(0.0);

    // Min input comes before the track.
    let min_input_x = label_x;
    let min_input_y = center_y - config.input_height / 2.0;
    let min_irect   = Rect::new(min_input_x, min_input_y, config.input_width, config.input_height);

    // Track starts after min input.
    let track_x = label_x + config.input_width + style.track_input_spacing();
    let track_rect = Rect::new(track_x, track_y, track_w, track_h);

    // Max input after track.
    let max_input_x = track_x + track_w + style.track_input_spacing();
    let max_input_y = center_y - config.input_height / 2.0;
    let max_irect   = Rect::new(max_input_x, max_input_y, config.input_width, config.input_height);

    // ── Track ────────────────────────────────────────────────────────────────
    let (x_min, x_max) = draw_dual_track(
        ctx, track_x, track_y, track_w, track_h, radius,
        view.min_value, view.max_value,
        config.min, config.max,
        settings,
    );

    // ── Handles ──────────────────────────────────────────────────────────────
    let r = style.handle_radius();
    let min_hot = view.hovered && view.active_handle == Some(DualSliderHandle::Min);
    let max_hot = view.hovered && view.active_handle == Some(DualSliderHandle::Max);
    let both_hot = view.hovered && view.active_handle.is_none();

    draw_handle(ctx, x_min, center_y, min_hot || both_hot, view.disabled, settings);
    draw_handle(ctx, x_max, center_y, max_hot || both_hot, view.disabled, settings);

    let min_handle_rect = Rect::new(x_min - r, center_y - r, r * 2.0, r * 2.0);
    let max_handle_rect = Rect::new(x_max - r, center_y - r, r * 2.0, r * 2.0);

    // ── Input boxes ──────────────────────────────────────────────────────────
    let input_settings = make_input_settings(style, theme);

    let draw_value_input = |ctx: &mut dyn RenderContext,
                             irect: Rect,
                             value: f64,
                             editing: Option<&crate::ui::widgets::atomic::slider::types::SliderEditingInfo<'_>>|
     -> Rect {
        let (text, cursor, selection, focused) = match editing {
            Some(ed) => (
                ed.text,
                ed.cursor,
                ed.selection_start.map(|s| (s, ed.cursor)),
                true,
            ),
            None => ("", 0, None, false),
        };

        let formatted;
        let display_text = if text.is_empty() && !focused {
            formatted = config.format_value(value);
            formatted.as_str()
        } else {
            text
        };

        let iv = InputView {
            text:        display_text,
            placeholder: "",
            cursor,
            selection,
            focused,
            disabled: view.disabled,
            input_type: InputType::Number,
        };
        draw_input(ctx, irect, WidgetState::Normal, &iv, &input_settings);
        irect
    };

    let min_rect_out = draw_value_input(ctx, min_irect, view.min_value, view.editing_min.as_ref());
    let max_rect_out = draw_value_input(ctx, max_irect, view.max_value, view.editing_max.as_ref());

    // ── Track info ────────────────────────────────────────────────────────────
    let track_info = SliderTrackInfo {
        track_x,
        track_width: track_w,
        track_y: center_y - r,
        track_height: r * 2.0,
        min_val: config.min,
        max_val: config.max,
    };

    DualSliderResult {
        full_rect: row_rect,
        track_rect,
        min_handle_rect,
        max_handle_rect,
        min_input_rect: Some(min_rect_out),
        max_input_rect: Some(max_rect_out),
        track_info,
    }
}

// ─── Variant 1.5 ─────────────────────────────────────────────────────────────

/// Render the manual line-width slider from `compare_settings`.
///
/// Uses toolbar theme colours. No input box — value is rendered as a
/// `"{:.1}px"` text label to the right of the track.
///
/// `track_area` is the full available rect for the slider row (label column
/// already subtracted by the caller). The function reserves `lw_label_reserved`
/// px on the right for the value label.
pub fn draw_line_width_slider(
    ctx: &mut dyn RenderContext,
    track_area: Rect,
    view: &LineWidthSliderView,
    settings: &SliderSettings,
) -> LineWidthSliderResult {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let handle_r = style.lw_handle_radius();
    let track_h  = style.track_height();
    let center_y = track_area.center_y();
    let track_y  = center_y - track_h / 2.0;
    let radius   = style.track_radius();

    let track_w  = (track_area.width - style.lw_label_reserved()).max(0.0);
    let track_x  = track_area.x;

    // ── Track background ─────────────────────────────────────────────────────
    ctx.set_fill_color(theme.toolbar_track_empty());
    ctx.fill_rounded_rect(track_x, track_y, track_w, track_h, radius);

    // ── Filled portion ────────────────────────────────────────────────────────
    let handle_x = value_to_x(view.value, view.min, view.max, track_x, track_w);
    let fill_w   = (handle_x - track_x).max(0.0);
    if fill_w > 0.0 {
        ctx.set_fill_color(theme.toolbar_track_filled());
        ctx.fill_rounded_rect(track_x, track_y, fill_w, track_h, radius);
    }

    // ── Handle circle ─────────────────────────────────────────────────────────
    ctx.set_fill_color(theme.toolbar_handle());
    ctx.fill_rounded_rect(
        handle_x - handle_r,
        center_y - handle_r,
        handle_r * 2.0,
        handle_r * 2.0,
        handle_r,
    );

    // ── Value label ───────────────────────────────────────────────────────────
    let label_x = track_x + track_w + style.lw_label_gap();
    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.set_fill_color(theme.toolbar_handle());
    ctx.fill_text(&format!("{:.1}px", view.value), label_x, center_y);

    // ── Hit rect (inflated by handle_r on each horizontal side) ──────────────
    let hit_rect = Rect::new(
        track_x - handle_r,
        center_y - handle_r,
        track_w + handle_r * 2.0,
        handle_r * 2.0,
    );

    let track_info = SliderTrackInfo {
        track_x,
        track_width: track_w,
        track_y: center_y - handle_r,
        track_height: handle_r * 2.0,
        min_val: view.min,
        max_val: view.max,
    };

    LineWidthSliderResult { hit_rect, track_info }
}

// ─── Legacy draw_slider (kept for existing callers) ──────────────────────────

/// Per-frame inputs for the low-level track-only renderer.
///
/// Kept for callers that use `draw_slider` directly without the row layout.
pub struct SliderView {
    pub kind: SliderType,
    pub hovered: bool,
    pub disabled: bool,
    pub dragging_handle: Option<DualSliderHandle>,
}

/// Minimal result from `draw_slider`.
#[derive(Debug, Default, Clone)]
pub struct SliderResult {
    pub track_rect: Rect,
    pub handle_x: f64,
    pub handle_max_x: f64,
}

/// Render only the track + handle(s) into `track_rect` (no label, no input).
///
/// Use `draw_single_slider` / `draw_dual_slider` for full-row rendering.
pub fn draw_slider(
    ctx: &mut dyn RenderContext,
    track_rect: Rect,
    _state: WidgetState,
    view: &SliderView,
    settings: &SliderSettings,
) -> SliderResult {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let track_y  = track_rect.y + track_rect.height / 2.0 - style.track_height() / 2.0;
    let track_x  = track_rect.x;
    let track_w  = track_rect.width;
    let track_h  = style.track_height();
    let radius   = style.track_radius();
    let center_y = track_rect.center_y();

    let (handle_x, handle_max_x) = match view.kind {
        SliderType::Single { value, min, max, .. } => {
            // Empty track
            ctx.set_fill_color(theme.track_empty());
            ctx.fill_rounded_rect(track_x, track_y, track_w, track_h, radius);

            let hx = value_to_x(value, min, max, track_x, track_w);
            let fill_w = (hx - track_x).max(0.0);
            if fill_w > 0.0 {
                ctx.set_fill_color(theme.accent());
                ctx.fill_rounded_rect(track_x, track_y, fill_w, track_h, radius);
            }
            (hx, 0.0)
        }
        SliderType::Dual { min_value, max_value, min, max, .. } => {
            ctx.set_fill_color(theme.track_empty());
            ctx.fill_rounded_rect(track_x, track_y, track_w, track_h, radius);

            let x_min = value_to_x(min_value, min, max, track_x, track_w);
            let x_max = value_to_x(max_value, min, max, track_x, track_w);
            let fill_w = (x_max - x_min).max(0.0);
            if fill_w > 0.0 {
                ctx.set_fill_color(theme.accent());
                ctx.fill_rounded_rect(x_min, track_y, fill_w, track_h, radius);
            }
            (x_min, x_max)
        }
    };

    // Handles
    let is_hovered = view.hovered;
    match view.kind {
        SliderType::Single { .. } => {
            draw_handle(ctx, handle_x, center_y, is_hovered, view.disabled, settings);
        }
        SliderType::Dual { .. } => {
            let min_hot = is_hovered && view.dragging_handle == Some(DualSliderHandle::Min);
            let max_hot = is_hovered && view.dragging_handle == Some(DualSliderHandle::Max);
            let both    = is_hovered && view.dragging_handle.is_none();
            draw_handle(ctx, handle_x,     center_y, min_hot || both, view.disabled, settings);
            draw_handle(ctx, handle_max_x, center_y, max_hot || both, view.disabled, settings);
        }
    }

    SliderResult { track_rect, handle_x, handle_max_x }
}
