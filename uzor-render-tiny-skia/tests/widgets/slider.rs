//! Golden grid (H1 Brief 10a-3): slider × 4 built-in sets × its own real
//! state set. `SingleSliderView` carries `hovered`/`disabled`; `draw_handle`
//! (`render.rs`) only reads those two — a "dragging" `bool` does not exist
//! anywhere on this widget's per-frame view types. Live dragging only
//! changes which `value` the caller threads through each frame
//! (`SliderDragState`'s own module doc: `floating_value` is the drag
//! preview), so a mid-drag frame renders through the exact same
//! `hovered = true` branch as a plain hover, just at a different handle
//! position — the "dragging" cell below models that (same halo, moved
//! value) rather than inventing a colour axis the widget doesn't have.
//! `draw_dual_slider` (variant 1.4) is the 4th cell: two independent
//! handles + two inline value boxes, a genuinely different layout.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::slider::{
    self, DefaultSliderStyle, DualSliderView, SingleSliderView, SliderConfig, SliderSettings,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(8.0, 8.0, 220.0, 32.0)
}

enum Variant {
    Single { value: f64, hovered: bool },
    Dual,
}

fn settings(set: BuiltinSet) -> SliderSettings {
    SliderSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::new(DefaultSliderStyle),
    }
}

fn render(set: BuiltinSet, variant: &Variant) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(236, 48, set);
    let settings = settings(set);
    match variant {
        Variant::Single { value, hovered } => {
            let config = SliderConfig::new(0.0, 100.0);
            let view = SingleSliderView {
                config: &config,
                value: *value,
                label: Some("Width"),
                editing: None,
                hovered: *hovered,
                disabled: false,
            };
            slider::draw_single_slider(&mut ctx, rect(), &view, &settings);
        }
        Variant::Dual => {
            let config = SliderConfig::new(0.0, 100.0);
            let view = DualSliderView {
                config: &config,
                min_value: 20.0,
                max_value: 70.0,
                label: Some("Range"),
                editing_min: None,
                editing_max: None,
                hovered: false,
                active_handle: None,
                disabled: false,
            };
            slider::draw_dual_slider(&mut ctx, rect(), &view, &settings);
        }
    }
    ctx
}

/// `(state name, variant)` — module doc explains why "dragging" reuses the
/// hover branch at a different value rather than a distinct colour.
const STATES: &[(&str, Variant)] = &[
    ("idle", Variant::Single { value: 40.0, hovered: false }),
    ("hover", Variant::Single { value: 40.0, hovered: true }),
    ("dragging", Variant::Single { value: 75.0, hovered: true }),
    ("dual", Variant::Dual),
];

#[test]
fn slider_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for (state_name, variant) in STATES {
            let ctx = render(set, variant);
            support::golden("slider", set, state_name, &ctx).expect("slider golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("slider");
}
