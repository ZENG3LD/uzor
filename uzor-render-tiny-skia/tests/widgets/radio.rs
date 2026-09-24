//! Golden grid (H1 Brief 10a-2): radio × 4 built-in sets × its own real
//! state set. `RadioGroupView`/`RadioOption` (the `Group` render kind — the
//! canonical vertical list, section 35) carry exactly two interaction-
//! relevant fields: `RadioOption::hovered` and `RadioGroupView::selected`
//! (an index match, not a bool per option) — so "unselected"/"selected"/
//! "hover" is the widget's entire real state set for a single-option row
//! (`selected` set to an out-of-range index yields the unselected ring for
//! the one option present, exactly like a real never-selected row).
//!
//! `RadioTheme::radio_disabled_overlay` exists as a colour slot but
//! `draw_radio`'s built-in `Group`/`Pair`/`Dot` variants never call it —
//! `WidgetState` is accepted by `draw_radio` only to forward to the
//! `Custom` escape hatch (module doc), so "disabled" cannot be produced by
//! any built-in render path and is out of scope here (matches the
//! `checkbox` golden's precedent of testing only the widget's real,
//! renderable state set).

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::radio::{self, RadioGroupView, RadioOption, RadioRenderKind, RadioSettings};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(0.0, 0.0, 200.0, 72.0)
}

fn render(set: BuiltinSet, selected: bool, hovered: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(200, 72, set);
    let settings = RadioSettings::default().with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))));
    let options = [RadioOption { label: "Label", description: "", hovered }];
    // Only one option exists — `selected: 1` is an out-of-range index, so it
    // never matches `i == 0` and the row renders as genuinely unselected.
    let view = RadioGroupView { options: &options, selected: if selected { 0 } else { 1 } };
    let kind = RadioRenderKind::Group { x: 12.0, y: 10.0, width: 176.0, view };
    radio::draw_radio(&mut ctx, rect(), WidgetState::Normal, &settings, &kind);
    ctx
}

/// `(state name, selected, hovered)` — radio's entire real state set (module doc).
const STATES: &[(&str, bool, bool)] = &[
    ("unselected", false, false),
    ("selected", true, false),
    ("hover", false, true),
];

#[test]
fn radio_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, selected, hovered) in STATES {
            let ctx = render(set, selected, hovered);
            support::golden("radio", set, state_name, &ctx).expect("radio golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("radio");
}
