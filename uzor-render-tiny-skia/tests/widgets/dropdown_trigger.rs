//! Golden grid (H1 Brief 10a-2): dropdown_trigger × 4 built-in sets × its
//! own real state set, using the `Field` render kind (`DropdownFieldView`
//! carries `hovered` and `open` — the single-zone trigger styled like a
//! form input, section 33). States: normal, hover, open.
//!
//! NOTE: `draw_dropdown_field`'s own background rule is
//! `if view.hovered || view.open { bg_hover } else { bg }` — hover and open
//! resolve to the identical background colour (no separate "menu is open"
//! visual exists yet in this render path). The "hover" and "open" golden
//! cells are therefore expected to be pixel-identical to each other; this
//! documents the widget's actual current behaviour rather than a test bug.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::dropdown_trigger::{self, DropdownFieldView, DropdownTriggerSettings};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

const FONT: &str = "13px sans-serif";

fn rect() -> Rect {
    Rect::new(10.0, 10.0, 140.0, 32.0)
}

fn render(set: BuiltinSet, hovered: bool, open: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(160, 52, set);
    let settings =
        DropdownTriggerSettings::default().with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))));
    let view = DropdownFieldView { current_label: "Value", open, hovered };
    dropdown_trigger::draw_dropdown_field(
        &mut ctx,
        rect(),
        &view,
        FONT,
        settings.field_style.as_ref(),
        settings.theme.as_ref(),
    );
    ctx
}

/// `(state name, hovered, open)` — dropdown_trigger's real `Field` state set.
const STATES: &[(&str, bool, bool)] = &[
    ("normal", false, false),
    ("hover", true, false),
    ("open", false, true),
];

#[test]
fn dropdown_trigger_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, hovered, open) in STATES {
            let ctx = render(set, hovered, open);
            support::golden("dropdown_trigger", set, state_name, &ctx)
                .expect("dropdown_trigger golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("dropdown_trigger");
}
