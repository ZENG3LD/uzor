//! Golden grid (H1 Brief 10a-3): tab × 4 built-in sets × its own real state
//! set, using the generic `draw_tab` dispatcher (`DefaultTabStyle`).
//! `TabView`/`TabConfig` carry `active`/`hovered`/`pressed`; `draw_tab`'s own
//! background match treats `hovered || pressed` identically (`render.rs`),
//! so "normal"/"hover"/"active" is the widget's entire real colour-axis
//! state set for this dispatcher (active also draws the left accent bar).

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::tab::{
    self, ChromeTabStyle, DefaultTabStyle, ModalHorizontalTabStyle, ModalSidebarTabStyle,
    TabConfig, TabSettings, TabView, TagsTabsSidebarTabStyle,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(4.0, 4.0, 112.0, 32.0)
}

fn render(set: BuiltinSet, active: bool, hovered: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(120, 40, set);
    let settings = TabSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::new(DefaultTabStyle),
        chrome: ChromeTabStyle::default(),
        modal_sidebar: ModalSidebarTabStyle::default(),
        modal_horizontal: ModalHorizontalTabStyle::default(),
        tags_sidebar: TagsTabsSidebarTabStyle::default(),
    };
    let mut config = TabConfig::new("tab-1", "Chart");
    if active {
        config = config.active();
    }
    let view = TabView { tab: &config, hovered, pressed: false, close_btn_hovered: false };
    tab::draw_tab(&mut ctx, rect(), &view, &settings);
    ctx
}

/// `(state name, active, hovered)` — tab's entire real colour-axis state
/// set for `draw_tab` (module doc).
const STATES: &[(&str, bool, bool)] = &[
    ("normal", false, false),
    ("hover", false, true),
    ("active", true, false),
];

#[test]
fn tab_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, active, hovered) in STATES {
            let ctx = render(set, active, hovered);
            support::golden("tab", set, state_name, &ctx).expect("tab golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("tab");
}
