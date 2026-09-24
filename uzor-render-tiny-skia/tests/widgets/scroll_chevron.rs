//! Golden grid (H1 Brief 10a-3): scroll_chevron × 4 built-in sets × its own
//! real state set. `ScrollChevronView` carries exactly `hovered`/`disabled`
//! (module doc on `draw_scroll_chevron_inner`'s own colour-priority ladder:
//! disabled > hovered > normal), so "normal"/"hover"/"disabled" is the
//! widget's entire real state set — hover additionally draws a rounded-rect
//! background halo behind the glyph.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::scroll_chevron::{
    self, ChevronDirection, DefaultScrollChevronStyle, ScrollChevronRenderKind,
    ScrollChevronSettings, ScrollChevronView,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(8.0, 8.0, 16.0, 16.0)
}

fn render(set: BuiltinSet, hovered: bool, disabled: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(32, 32, set);
    let settings = ScrollChevronSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::new(DefaultScrollChevronStyle),
    };
    let view = ScrollChevronView { direction: ChevronDirection::Right, hovered, disabled };
    scroll_chevron::draw_scroll_chevron(
        &mut ctx,
        rect(),
        WidgetState::Normal,
        &view,
        &settings,
        &ScrollChevronRenderKind::Default,
    );
    ctx
}

/// `(state name, hovered, disabled)` — scroll_chevron's entire real state
/// set (module doc).
const STATES: &[(&str, bool, bool)] = &[
    ("normal", false, false),
    ("hover", true, false),
    ("disabled", false, true),
];

#[test]
fn scroll_chevron_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, hovered, disabled) in STATES {
            let ctx = render(set, hovered, disabled);
            support::golden("scroll_chevron", set, state_name, &ctx)
                .expect("scroll_chevron golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("scroll_chevron");
}
