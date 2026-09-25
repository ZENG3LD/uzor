//! Golden grid (H1 Brief 10a-composites): chrome × 4 built-in sets × its
//! `Default` kind (tabs + drag zone + window controls, module doc table).
//! Fixed 3-tab strip, one tab active via `ChromeView::active_tab_id` (module
//! doc: `ChromeTabConfig::active` itself is dead — `draw_chrome` computes
//! "active" solely from `active_tab_id == tab.id`). Optional side buttons
//! (new-tab, menu, new-window, close-window) are all off so the strip is
//! exactly tabs + the min/max/close trio — the minimum "window controls"
//! surface the composite draws. `ChromeState::tabs_state[i].hovered` and
//! `ChromeState::hovered` are the widget's whole interactive axis — no
//! `InputCoordinator`, no wall clock — so a fixed hover flag per cell is a
//! deterministic hover sample, never a live pointer position.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, Tokens, TokenTheme};
use uzor::types::Rect;
use uzor::ui::widgets::composite::chrome::{
    self, ChromeHit, ChromeRenderKind, ChromeSettings, ChromeState, ChromeTabConfig, ChromeView,
    DefaultChromeStyle,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

const TAB_IDS: &[&str] = &["overview", "chart", "trades"];

fn tabs() -> Vec<ChromeTabConfig<'static>> {
    vec![
        ChromeTabConfig {
            id: "overview",
            label: "Overview",
            icon: None,
            color_tag: None,
            closable: false,
            active: false,
        },
        ChromeTabConfig {
            id: "chart",
            label: "Chart",
            icon: None,
            color_tag: None,
            closable: false,
            active: false,
        },
        ChromeTabConfig {
            id: "trades",
            label: "Trades",
            icon: None,
            color_tag: None,
            closable: false,
            active: false,
        },
    ]
}

/// `(state name, hovered tab index, close-app button hovered)` —
/// `ChromeState`'s whole interactive axis for a fixed-content golden
/// (module doc). Index 1 ("Chart") is not the active tab, so hovering it
/// exercises the hover branch distinctly from the active branch.
const STATES: &[(&str, Option<usize>, bool)] =
    &[("normal", None, false), ("tab_hovered", Some(1), false), ("close_hovered", None, true)];

fn render(set: BuiltinSet, hovered_tab: Option<usize>, close_hovered: bool) -> TinySkiaCpuRenderContext {
    let tabs = tabs();
    let view = ChromeView {
        tabs: &tabs,
        active_tab_id: Some("overview"),
        show_new_tab_btn: false,
        show_menu_btn: false,
        show_new_window_btn: false,
        show_close_window_btn: false,
        is_maximized: false,
        menu_left: false,
        show_maximize: true,
        cursor_x: 0.0,
        cursor_y: 0.0,
        time_ms: 0.0,
    };
    let settings = ChromeSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::<DefaultChromeStyle>::default(),
    };
    let kind = ChromeRenderKind::Default;

    let mut state = ChromeState::new();
    state.sync_tabs(TAB_IDS);
    if let Some(i) = hovered_tab {
        if let Some(ts) = state.tabs_state.get_mut(i) {
            ts.hovered = true;
        }
    }
    if close_hovered {
        state.hovered = ChromeHit::CloseBtn;
    }

    let (w, h) = chrome::measure(&view, &state, &settings, &kind);
    let mut ctx = support::canvas(w.ceil() as u32, h.ceil() as u32, set);
    let rect = Rect::new(0.0, 0.0, w, h);
    chrome::draw_chrome(&mut ctx, rect, &state, &view, &settings, &kind);
    ctx
}

#[test]
fn chrome_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, hovered_tab, close_hovered) in STATES {
            let ctx = render(set, hovered_tab, close_hovered);
            support::golden("chrome", set, state_name, &ctx).expect("chrome golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("chrome");
}
