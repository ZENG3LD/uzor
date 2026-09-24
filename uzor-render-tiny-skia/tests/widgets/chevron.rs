//! Golden grid (H1 Brief 10a-1): chevron × 4 built-in sets × its own real
//! interaction flags (`ChevronView::{hovered,pressed,disabled}`, colour
//! priority disabled > pressed > active > hover > normal per
//! `chevron::render::draw_chevron`'s own doc comment).

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::chevron::{
    self, ChevronSettings, ChevronView, DefaultChevronStyle,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(16.0, 16.0, 32.0, 32.0)
}

fn render(set: BuiltinSet, hovered: bool, pressed: bool, disabled: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(64, 64, set);
    let settings = ChevronSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::new(DefaultChevronStyle),
    };
    let view = ChevronView { hovered, pressed, disabled, ..ChevronView::default() };
    chevron::draw_chevron(&mut ctx, rect(), &view, &settings);
    ctx
}

/// `(state name, hovered, pressed, disabled)` — the 4 meaningful states
/// `draw_chevron`'s colour-priority ladder actually distinguishes.
const STATES: &[(&str, bool, bool, bool)] = &[
    ("normal", false, false, false),
    ("hover", true, false, false),
    ("pressed", true, true, false),
    ("disabled", false, false, true),
];

#[test]
fn chevron_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, hovered, pressed, disabled) in STATES {
            let ctx = render(set, hovered, pressed, disabled);
            support::golden("chevron", set, state_name, &ctx).expect("chevron golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("chevron");
}
