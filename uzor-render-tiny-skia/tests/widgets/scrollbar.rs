//! Golden grid (H1 Brief 10a-2): scrollbar × 4 built-in sets × its own real
//! state set, using `StandardScrollbarStyle` (opacity-gated, no track bg —
//! `render.rs`'s own opacity/colour table). Content taller than the
//! viewport so the thumb actually renders (content fitting the viewport is
//! the widget's "nothing to draw" early-return, not a visual state).
//!
//! UPDATE (H1 Brief 10a-3 item 0b): `draw_scrollbar` now reads
//! `theme.thumb_hover()` for `HandleHovered` and `theme.thumb_active()` for
//! `Dragging` — previously both read `thumb_active()`, leaving
//! `thumb_hover()` dead. The "hover" and "dragging" golden cells stay
//! pixel-identical in every built-in set today only because
//! `ScrollbarTokens`' own default table aliases both fields to the same
//! semantic role (`ScrollbarTokens`'s own module doc) — a token-table fact,
//! not a code defect; the two draw paths are independent now.

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
