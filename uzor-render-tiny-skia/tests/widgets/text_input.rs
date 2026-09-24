//! Golden grid (H1 Brief 10a-4): text_input × 4 built-in sets × its own real
//! state set. `InputView` carries `text`/`placeholder`/`cursor`/`selection`/
//! `focused`/`disabled` — empty-with-placeholder, filled, focused-with-
//! selection, and disabled are the widget's whole meaningful state set.
//! `draw_input` never draws the caret itself (module doc: "Cursor itself is
//! not drawn here — caller invokes `draw_input_cursor` after consulting
//! blink visibility") — the real widget leaves blink timing to the caller
//! (`TextFieldStore::cursor_visible(now_ms)`), so a golden that called that
//! with a live clock would be non-deterministic. This test never touches
//! `now_ms` at all: the "focused" cell always invokes `draw_input_cursor`
//! directly, i.e. a fixed blink phase (permanently visible), not a wall-clock
//! sample.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::text_input::{self, InputType, InputView, TextFieldConfig, TextInputSettings};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(8.0, 8.0, 220.0, 30.0)
}

fn settings(set: BuiltinSet) -> TextInputSettings {
    TextInputSettings::with_config(TextFieldConfig::text())
        .with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))))
}

/// One cell's worth of `InputView` data (module doc: the widget's whole
/// meaningful state set).
struct Variant {
    text: &'static str,
    placeholder: &'static str,
    cursor: usize,
    selection: Option<(usize, usize)>,
    focused: bool,
    disabled: bool,
}

/// `(state name, variant)` — named after `InputView`'s own fields, the
/// widget's whole meaningful state set (module doc).
const STATES: &[(&str, Variant)] = &[
    (
        "empty",
        Variant {
            text: "",
            placeholder: "Search…",
            cursor: 0,
            selection: None,
            focused: false,
            disabled: false,
        },
    ),
    (
        "filled",
        Variant {
            text: "Hello world",
            placeholder: "",
            cursor: 11,
            selection: None,
            focused: false,
            disabled: false,
        },
    ),
    (
        "focused",
        Variant {
            text: "Hello world",
            placeholder: "",
            cursor: 5,
            selection: Some((0, 5)),
            focused: true,
            disabled: false,
        },
    ),
    (
        "disabled",
        Variant {
            text: "Disabled",
            placeholder: "",
            cursor: 0,
            selection: None,
            focused: false,
            disabled: true,
        },
    ),
];

fn render(set: BuiltinSet, variant: &Variant) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(236, 46, set);
    let settings = settings(set);
    let view = InputView {
        text: variant.text,
        placeholder: variant.placeholder,
        cursor: variant.cursor,
        selection: variant.selection,
        focused: variant.focused,
        disabled: variant.disabled,
        input_type: InputType::Text,
    };
    let result = text_input::draw_input(&mut ctx, rect(), WidgetState::Normal, &view, &settings);
    if variant.focused {
        // Fixed blink phase (always visible) — deterministic, never wall-clock.
        text_input::draw_input_cursor(
            &mut ctx,
            result.cursor_x,
            result.cursor_y,
            result.cursor_height,
            settings.style.cursor_width(),
            settings.theme.cursor(),
        );
    }
    ctx
}

#[test]
fn text_input_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for (state_name, variant) in STATES {
            let ctx = render(set, variant);
            support::golden("text_input", set, state_name, &ctx).expect("text_input golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("text_input");
}
