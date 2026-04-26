//! Scrollbar rendering — three style-driven variants, all driven by one fn.
//!
//! ## Opacity model (mlc parity)
//!
//! | `ScrollbarVisualState` | opacity |
//! |------------------------|---------|
//! | `Hidden`               | skipped — early return, nothing drawn |
//! | `Dormant`              | 0.0 — early return after opacity check |
//! | `Active`               | 0.5 |
//! | `HandleHovered`        | 0.8 |
//! | `Dragging`             | 0.8 |
//!
//! `CompactScrollbarStyle` bypasses opacity gating — always fully opaque (1.0).
//! `SignalScrollbarStyle` also ignores `ScrollbarVisualState` and applies
//! fixed alpha via hex suffix directly on the separator colour string.

use crate::render::RenderContext;
use crate::types::Rect;

use super::style::ScrollbarStyle;
use super::theme::ScrollbarTheme;

/// Visual interaction state forwarded from input layer to the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarVisualState {
    /// Scrollbar is not rendered at all.
    #[default]
    Hidden,
    /// Content fits; scrollbar is logically present but invisible (opacity 0.0).
    Dormant,
    /// Scrollbar is visible at rest opacity (0.5).
    Active,
    /// Cursor is over the thumb (0.8).
    HandleHovered,
    /// Thumb is being dragged (0.8).
    Dragging,
}

/// Geometry returned by `draw_scrollbar` — callers use this to register hit zones.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScrollbarResult {
    /// Rendered track rect (inset by `style.track_padding()`).
    pub track_rect: Rect,
    /// Rendered thumb rect.
    pub thumb_rect: Rect,
    /// Updated scroll offset (in pixels).  Equals the input offset except when
    /// `drag_pos_y` overrides it during live drag.
    pub scroll_offset: f64,
    /// `true` if offset was adjusted by a drag override this frame.
    pub dragged: bool,
}

/// Parameters for one `draw_scrollbar` call.
pub struct ScrollbarView<'a> {
    /// Total scrollable content height in pixels.
    pub content_height: f64,
    /// Visible viewport height in pixels.
    pub viewport_height: f64,
    /// Current scroll offset in pixels.
    pub scroll_offset: f64,
    /// Interaction state — drives opacity and colour selection.
    pub state: ScrollbarVisualState,
    /// If `Some(y)`, live-drag override: recompute thumb position from absolute Y.
    /// The returned `scroll_offset` will reflect the dragged position.
    pub drag_pos_y: Option<f64>,
    /// Style preset (Standard / Compact / Signal).
    pub style: &'a dyn ScrollbarStyle,
    /// Colour palette.
    pub theme: &'a dyn ScrollbarTheme,
}

/// Draw a vertical scrollbar into `rect` and return geometry + updated offset.
///
/// `rect` is the full allocated column (width = `style.track_thickness()`).
/// The function insets it by `style.track_padding()` before drawing.
pub fn draw_scrollbar(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ScrollbarView<'_>,
) -> ScrollbarResult {
    let style = view.style;
    let theme = view.theme;

    // ── Early exit: hidden ────────────────────────────────────────────────────
    if matches!(view.state, ScrollbarVisualState::Hidden) {
        return ScrollbarResult {
            scroll_offset: view.scroll_offset,
            track_rect: rect,
            ..Default::default()
        };
    }

    // ── Content fits → nothing to show ───────────────────────────────────────
    if view.content_height <= view.viewport_height {
        return ScrollbarResult {
            scroll_offset: view.scroll_offset,
            track_rect: rect,
            ..Default::default()
        };
    }

    // ── Opacity (Standard / Compact differ; Signal overrides colour directly) ─
    // CompactScrollbarStyle: always 1.0 (no opacity gating).
    // StandardScrollbarStyle: opacity-gated per state.
    // SignalScrollbarStyle: handled separately — it draws track bg and uses hex
    // suffix alpha instead.
    let opacity = match view.state {
        ScrollbarVisualState::Hidden  => 0.0,
        ScrollbarVisualState::Dormant => 0.0,
        ScrollbarVisualState::Active  => 0.5,
        ScrollbarVisualState::HandleHovered | ScrollbarVisualState::Dragging => 0.8,
    };

    // CompactScrollbarStyle bypasses the opacity gate (always opaque).
    // Detect by checking `track_padding` == 0.0 AND `draw_track_bg` == false
    // AND `thumb_min_length` == 24.0 — but that's fragile.  Instead expose it
    // via `draw_track_bg`=false plus the caller sets state=Active for compact.
    // Simplest correct rule from mlc: compact never gates opacity.
    // We use a secondary opacity — if style declares no track bg AND radius < 4
    // AND min_length < 30, treat as compact → opacity=1.0.
    let effective_opacity = if style.thumb_min_length() < 30.0 && !style.draw_track_bg() {
        // Compact variant: always fully opaque
        1.0
    } else {
        opacity
    };

    if effective_opacity <= 0.0 {
        return ScrollbarResult {
            scroll_offset: view.scroll_offset,
            track_rect: rect,
            ..Default::default()
        };
    }

    // ── Track rect ────────────────────────────────────────────────────────────
    let pad = style.track_padding();
    let track_rect = Rect::new(
        rect.x + pad,
        rect.y + pad,
        rect.width - pad * 2.0,
        rect.height - pad * 2.0,
    );

    // ── Optional track background (Signal variant) ────────────────────────────
    if style.draw_track_bg() {
        // mlc: `separator + "20"` ≈ 12 % opacity hex suffix.
        // We replicate that by appending "20" to the hex colour string.
        let track_bg = append_hex_alpha(theme.track_bg(), 0x20);
        ctx.set_fill_color(&track_bg);
        ctx.fill_rect(track_rect.x, track_rect.y, track_rect.width, track_rect.height);
    }

    // ── Thumb geometry ────────────────────────────────────────────────────────
    let visible_ratio = (view.viewport_height / view.content_height).clamp(0.0, 1.0);
    let max_scroll    = (view.content_height - view.viewport_height).max(0.0);
    let scroll_ratio  = if max_scroll > 0.0 {
        (view.scroll_offset / max_scroll).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let thumb_len = (track_rect.height * visible_ratio)
        .max(style.thumb_min_length())
        .min(track_rect.height);
    let available  = (track_rect.height - thumb_len).max(0.0);
    let mut thumb_y = track_rect.y + scroll_ratio * available;
    let mut offset  = view.scroll_offset;

    // Live drag override
    if let Some(y) = view.drag_pos_y {
        let new_ratio = ((y - track_rect.y - thumb_len / 2.0) / available.max(1.0)).clamp(0.0, 1.0);
        offset   = new_ratio * max_scroll;
        thumb_y  = track_rect.y + new_ratio * available;
    }

    let thumb_rect = Rect::new(track_rect.x, thumb_y, track_rect.width, thumb_len);

    // ── Thumb colour ──────────────────────────────────────────────────────────
    let thumb_color = if style.draw_track_bg() {
        // Signal variant: fixed hex-suffix opacity, no state-based colour switch.
        // mlc: `separator + "80"` ≈ 50 % opacity.
        let s = append_hex_alpha(theme.thumb_normal(), 0x80);
        ctx.set_fill_color(&s);
        ctx.fill_rect(thumb_rect.x, thumb_rect.y, thumb_rect.width, thumb_rect.height);
        return ScrollbarResult {
            track_rect,
            thumb_rect,
            scroll_offset: offset,
            dragged: view.drag_pos_y.is_some(),
        };
    } else {
        match view.state {
            ScrollbarVisualState::HandleHovered | ScrollbarVisualState::Dragging => {
                theme.thumb_active()
            }
            _ => theme.thumb_normal(),
        }
    };

    // ── Draw thumb (Standard / Compact) ──────────────────────────────────────
    ctx.set_fill_color_alpha(thumb_color, effective_opacity);
    ctx.fill_rounded_rect(
        thumb_rect.x,
        thumb_rect.y,
        thumb_rect.width,
        thumb_rect.height,
        style.thumb_radius(),
    );
    ctx.reset_alpha();

    ScrollbarResult {
        track_rect,
        thumb_rect,
        scroll_offset: offset,
        dragged: view.drag_pos_y.is_some(),
    }
}

// ── Convenience wrappers ──────────────────────────────────────────────────────

/// Draw using `StandardScrollbarStyle` + `DefaultScrollbarTheme` (sidebar default).
pub fn draw_scrollbar_standard(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    content_height: f64,
    viewport_height: f64,
    scroll_offset: f64,
    state: ScrollbarVisualState,
    drag_pos_y: Option<f64>,
) -> ScrollbarResult {
    use super::style::StandardScrollbarStyle;
    use super::theme::DefaultScrollbarTheme;
    let style = StandardScrollbarStyle;
    let theme = DefaultScrollbarTheme;
    draw_scrollbar(ctx, rect, &ScrollbarView {
        content_height,
        viewport_height,
        scroll_offset,
        state,
        drag_pos_y,
        style: &style,
        theme: &theme,
    })
}

/// Draw using `CompactScrollbarStyle` + `DefaultScrollbarTheme` (profile-manager).
pub fn draw_scrollbar_compact(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    content_height: f64,
    viewport_height: f64,
    scroll_offset: f64,
    drag_pos_y: Option<f64>,
) -> ScrollbarResult {
    use super::style::CompactScrollbarStyle;
    use super::theme::DefaultScrollbarTheme;
    let style = CompactScrollbarStyle;
    let theme = DefaultScrollbarTheme;
    draw_scrollbar(ctx, rect, &ScrollbarView {
        content_height,
        viewport_height,
        scroll_offset,
        // Compact is always active-visible; no state-based opacity.
        state: ScrollbarVisualState::Active,
        drag_pos_y,
        style: &style,
        theme: &theme,
    })
}

/// Draw using `SignalScrollbarStyle` + `DefaultScrollbarTheme` (signal-group).
pub fn draw_scrollbar_signal(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    content_height: f64,
    viewport_height: f64,
    scroll_offset: f64,
    drag_pos_y: Option<f64>,
) -> ScrollbarResult {
    use super::style::SignalScrollbarStyle;
    use super::theme::DefaultScrollbarTheme;
    let style = SignalScrollbarStyle;
    let theme = DefaultScrollbarTheme;
    draw_scrollbar(ctx, rect, &ScrollbarView {
        content_height,
        viewport_height,
        scroll_offset,
        state: ScrollbarVisualState::Active,
        drag_pos_y,
        style: &style,
        theme: &theme,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Append a two-digit hex byte as alpha suffix to a `#rrggbb` string.
/// Returns the original string unchanged if it is not a 7-char `#rrggbb`.
fn append_hex_alpha(color: &str, alpha_byte: u8) -> String {
    if color.starts_with('#') && color.len() == 7 {
        format!("{}{:02x}", color, alpha_byte)
    } else {
        color.to_owned()
    }
}
