//! Text input rendering — ported from `mlc/chart/src/ui/widgets/input.rs`.
//!
//! Pure math: takes data + theme + style + state, emits draw calls
//! through `RenderContext`. No I/O, no platform code.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetState};

use super::settings::TextInputSettings;
use super::types::InputType;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn rgba_to_hex(c: [u8; 4]) -> String {
    if c[3] == 255 {
        format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3])
    }
}

fn char_idx_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn safe_char_slice(s: &str, char_start: usize, char_end: usize) -> &str {
    let a = char_idx_to_byte_idx(s, char_start);
    let b = char_idx_to_byte_idx(s, char_end);
    &s[a..b]
}

// ─── Per-frame rendering inputs ─────────────────────────────────────────────

/// What the caller hands to `draw_input` each frame. Pulled from
/// `TextFieldStore` (state.rs) and the widget's `TextInputType`.
pub struct InputView<'a> {
    /// Visible text. Caller is responsible for masking passwords if needed
    /// before passing — `display_value()` on `TextInputType` does this.
    pub text: &'a str,
    /// Placeholder shown when `text` is empty.
    pub placeholder: &'a str,
    /// Cursor position in **chars**.
    pub cursor: usize,
    /// Optional selection range in chars (start <= end already-sorted).
    pub selection: Option<(usize, usize)>,
    /// Has keyboard focus this frame.
    pub focused: bool,
    /// Disabled fields render in disabled colours and ignore selection.
    pub disabled: bool,
    /// Visual variant (Text / Number / Search / Password).
    pub input_type: InputType,
}

/// What the renderer returns. Saved by the caller for click→cursor
/// hit-testing and for the cursor blink draw on the next frame.
#[derive(Debug, Default, Clone)]
pub struct InputResult {
    /// Inset rectangle that contains the actual text (after padding).
    pub text_rect: Rect,
    /// Whether `state` was Hovered/Pressed (mirrors `is_hovered()`).
    pub hovered: bool,
    /// Cursor x in screen coords (already accounts for scroll offset).
    pub cursor_x: f64,
    /// Cursor y (top of the cursor stripe).
    pub cursor_y: f64,
    /// Cursor stripe height.
    pub cursor_height: f64,
    /// Pre-computed screen x for every character boundary
    /// (`len() == char_count + 1`). Use with `cursor_from_char_positions`
    /// at click time so we don't need a `RenderContext`.
    pub char_x_positions: Vec<f64>,
}

// ─── Public render entry point ──────────────────────────────────────────────

/// Render the input box (background, border, selection, text, placeholder)
/// and compute the cursor position. **Cursor itself is not drawn here** —
/// caller invokes `draw_input_cursor` after consulting blink visibility.
pub fn draw_input(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    state: WidgetState,
    view: &InputView,
    settings: &TextInputSettings,
) -> InputResult {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let effective_state = if view.disabled {
        WidgetState::Disabled
    } else {
        state
    };

    // ─── Colour selection by state ──────────────────────────────────────────
    let (bg, border, text_color) = match effective_state {
        WidgetState::Disabled => {
            (theme.bg_disabled(), theme.border_normal(), theme.text_disabled())
        }
        _ if view.focused => {
            (theme.bg_normal(), theme.border_focused(), theme.text_normal())
        }
        WidgetState::Hovered | WidgetState::Pressed => {
            (theme.bg_normal(), theme.border_hover(), theme.text_normal())
        }
        // Normal / Active / Toggled — text inputs don't differentiate
        _ => (theme.bg_normal(), theme.border_normal(), theme.text_normal()),
    };

    let radius = style.radius();

    // Background
    ctx.set_fill_color(&rgba_to_hex(bg));
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    // Border (focused = thicker)
    let border_width = if view.focused {
        style.border_width_focused()
    } else {
        style.border_width_normal()
    };
    ctx.set_stroke_color(&rgba_to_hex(border));
    ctx.set_stroke_width(border_width);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    // Inset for text. `text_rect.x` is the text origin; vertical extents fall
    // back to the original `rect` because large symmetric padding can collapse
    // `text_rect.height` to zero.
    let text_rect = rect.inset(style.padding());

    // Font + alignment
    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);

    let text_y = rect.center_y();
    let display_text = view.text;
    let char_count = display_text.chars().count();

    // ─── Scroll-offset calculation ──────────────────────────────────────────
    // When text is wider than the visible area, slide it so the cursor stays
    // in view. Mirrors mlc logic verbatim.
    let available_width = text_rect.width.max(0.0);
    let text_width = if display_text.is_empty() {
        0.0
    } else {
        ctx.measure_text(display_text)
    };

    let safe_cursor = view.cursor.min(char_count);
    let text_before_cursor = safe_char_slice(display_text, 0, safe_cursor);
    let cursor_offset_from_text_start = ctx.measure_text(text_before_cursor);

    let cursor_margin = style.cursor_margin();
    let scroll_offset_x = if text_width <= available_width {
        0.0
    } else {
        let ideal = cursor_offset_from_text_start - (available_width - cursor_margin);
        let max_scroll = (text_width - available_width).max(0.0);
        ideal.max(0.0).min(max_scroll)
    };

    // ─── Selection highlight ────────────────────────────────────────────────
    if !view.disabled {
        if let Some((sel_start, sel_end)) = view.selection {
            let s = sel_start.min(char_count);
            let e = sel_end.min(char_count);
            if s != e {
                let before = safe_char_slice(display_text, 0, s);
                let selected = safe_char_slice(display_text, s, e);
                let sel_x = text_rect.x - scroll_offset_x + ctx.measure_text(before);
                let sel_w = ctx.measure_text(selected);

                ctx.save();
                ctx.clip_rect(text_rect.x, rect.y, available_width, rect.height);
                ctx.set_fill_color(&rgba_to_hex(theme.selection()));
                ctx.fill_rect(sel_x, rect.y, sel_w, rect.height);
                ctx.restore();
            }
        }
    }

    // ─── Text or placeholder ────────────────────────────────────────────────
    ctx.save();
    ctx.clip_rect(text_rect.x, rect.y, available_width, rect.height);
    if display_text.is_empty() && !view.placeholder.is_empty() {
        ctx.set_fill_color(&rgba_to_hex(theme.placeholder()));
        ctx.fill_text(view.placeholder, text_rect.x, text_y);
    } else {
        ctx.set_fill_color(&rgba_to_hex(text_color));
        ctx.fill_text(display_text, text_rect.x - scroll_offset_x, text_y);
    }
    ctx.restore();

    // ─── Cursor geometry (drawn separately by caller via draw_input_cursor) ─
    let cursor_x = text_rect.x - scroll_offset_x + cursor_offset_from_text_start;
    let cursor_height = style.font_size() * 1.2;
    let cursor_y = text_y - cursor_height / 2.0;

    // ─── Pre-compute char-boundary x for click→cursor ──────────────────────
    let mut char_x_positions = Vec::with_capacity(char_count + 1);
    for i in 0..=char_count {
        let slice = safe_char_slice(display_text, 0, i);
        char_x_positions.push(text_rect.x - scroll_offset_x + ctx.measure_text(slice));
    }

    InputResult {
        text_rect,
        hovered: effective_state.is_hovered(),
        cursor_x,
        cursor_y,
        cursor_height,
        char_x_positions,
    }
}

/// Draw the blinking caret. Caller decides visibility via
/// `TextFieldStore::cursor_visible(now_ms)`.
pub fn draw_input_cursor(
    ctx: &mut dyn RenderContext,
    cursor_x: f64,
    cursor_y: f64,
    cursor_height: f64,
    width: f64,
    color: [u8; 4],
) {
    ctx.set_fill_color(&rgba_to_hex(color));
    ctx.fill_rect(cursor_x, cursor_y, width, cursor_height);
}

/// Map a click x-coordinate to the closest character boundary.
/// Use the `char_x_positions` array returned by `draw_input` so this can
/// run without a `RenderContext`.
pub fn cursor_from_char_positions(char_x_positions: &[f64], click_x: f64) -> usize {
    if char_x_positions.is_empty() {
        return 0;
    }
    let mut best_idx = 0;
    let mut best_dist = f64::MAX;
    for (i, &x) in char_x_positions.iter().enumerate() {
        let d = (click_x - x).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    best_idx
}
