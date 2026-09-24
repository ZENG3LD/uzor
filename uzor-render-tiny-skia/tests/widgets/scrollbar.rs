//! Golden grid (H1 Brief 10a-2): scrollbar × 4 built-in sets × its own real
//! state set, using `StandardScrollbarStyle` (opacity-gated, no track bg —
//! `render.rs`'s own opacity/colour table). Content taller than the
//! viewport so the thumb actually renders (content fitting the viewport is
//! the widget's "nothing to draw" early-return, not a visual state).
//!
//! NOTE: per `draw_scrollbar`'s own table, `HandleHovered` and `Dragging`
//! both resolve to `(theme.thumb_active(), 0.8 opacity)` — mlc parity
//! (module doc), not a colour axis this widget currently distinguishes. The
//! "hover" and "dragging" golden cells are therefore expected to be
//! pixel-identical to each other; `ScrollbarTheme::thumb_hover` exists as a
//! colour slot but `draw_scrollbar` never reads it.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::scrollbar::{self, ScrollbarView, ScrollbarVisualState, StandardScrollbarStyle};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(16.0, 10.0, 8.0, 200.0)
}

fn render(set: BuiltinSet, state: ScrollbarVisualState) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(40, 220, set);
    let style = StandardScrollbarStyle;
    let theme = TokenTheme::new(Tokens::builtin(set));
    let view = ScrollbarView {
        content_height: 400.0,
        viewport_height: 200.0,
        scroll_offset: 50.0,
        state,
        drag_pos_y: None,
        style: &style,
        theme: &theme,
    };
    scrollbar::draw_scrollbar(&mut ctx, rect(), &view);
    ctx
}

/// `(state name, ScrollbarVisualState)` — scrollbar's real, non-early-return
/// state set (module doc on the `HandleHovered`/`Dragging` duplication).
const STATES: &[(&str, ScrollbarVisualState)] = &[
    ("normal", ScrollbarVisualState::Active),
    ("hover", ScrollbarVisualState::HandleHovered),
    ("dragging", ScrollbarVisualState::Dragging),
];

#[test]
fn scrollbar_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, state) in STATES {
            let ctx = render(set, state);
            support::golden("scrollbar", set, state_name, &ctx).expect("scrollbar golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("scrollbar");
}
