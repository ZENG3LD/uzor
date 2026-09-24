//! Golden grid (H1 Brief 10a-1): color_swatch × 4 built-in sets × 4 states
//! (normal, hover, selected, transparent). `IndicatorSwatchStyle` is used
//! for the first three so `hover_expand`/`selected_border_width` actually
//! produce a visible difference (`SimpleSwatchStyle`'s `hover_expand() ==
//! 0.0` would make "hover" pixel-identical to "normal"); the transparent
//! state uses `ColorSwatchRenderKind::WithTransparency` with a
//! semi-transparent fill colour so the checkerboard shows through.
//!
//! Fill colour is a deliberate orange, not blue: `color_swatch_selected_border`
//! aliases `accent.default`, which is the exact same blue (`#2962ff`) as
//! `accent.default` happens to equal in 3 of the 4 built-in sets — an
//! accent-blue swatch would make the "selected" border invisible against its
//! own fill. Picking a colour that never coincides with a theme role is the
//! correct test fix (never loosen the tolerance) for this kind of collision.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::color_swatch::{
    self, ColorSwatchRenderKind, ColorSwatchSettings, ColorSwatchView, IndicatorSwatchStyle,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

const OPAQUE: [u8; 4] = [230, 126, 34, 255];
const TRANSLUCENT: [u8; 4] = [230, 126, 34, 140];

fn rect() -> Rect {
    Rect::new(20.0, 20.0, 20.0, 20.0)
}

fn render(
    set: BuiltinSet,
    kind: &ColorSwatchRenderKind<'_>,
    color: [u8; 4],
    hovered: bool,
    selected: bool,
) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(64, 64, set);
    let settings = ColorSwatchSettings::default()
        .with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))))
        .with_style(Box::new(IndicatorSwatchStyle));
    let view = ColorSwatchView {
        color,
        hovered,
        selected,
        show_transparency: false,
        border_color_override: None,
    };
    color_swatch::draw_color_swatch(&mut ctx, rect(), WidgetState::Normal, &view, &settings, kind);
    ctx
}

#[test]
fn color_swatch_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        let cases: [(&str, ColorSwatchRenderKind<'static>, [u8; 4], bool, bool); 4] = [
            ("normal", ColorSwatchRenderKind::Indicator, OPAQUE, false, false),
            ("hover", ColorSwatchRenderKind::Indicator, OPAQUE, true, false),
            ("selected", ColorSwatchRenderKind::Indicator, OPAQUE, false, true),
            ("transparent", ColorSwatchRenderKind::WithTransparency, TRANSLUCENT, false, false),
        ];
        for (state_name, kind, color, hovered, selected) in cases {
            let ctx = render(set, &kind, color, hovered, selected);
            support::golden("color_swatch", set, state_name, &ctx).expect("color_swatch golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("color_swatch");
}
