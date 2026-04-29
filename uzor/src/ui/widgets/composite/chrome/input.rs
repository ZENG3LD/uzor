//! Chrome input helpers.
//!
//! Re-exports `register_input_coordinator_chrome` from `render.rs` and adds:
//! - `chrome_hit_test` — converts a pointer position to a `ChromeHit`
//! - `handle_chrome_action` — maps a `ChromeHit` to a `ChromeAction`

pub use super::render::register_input_coordinator_chrome;

use super::render::register_context_manager_chrome;

use super::settings::ChromeSettings;
use super::state::ChromeState;
use super::types::{ChromeAction, ChromeHit, ChromeRenderKind, ChromeView, ResizeCorner};
use crate::core::types::Rect;
use crate::docking::panels::DockPanel;
use crate::input::LayerId;
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::WidgetId;

/// Level-3 registration: register and draw the chrome composite using rects
/// resolved from `LayoutManager`.
///
/// Returns `None` if chrome has not been solved yet (e.g. `solve()` not called
/// or chrome slot is hidden).
pub fn register_layout_manager_chrome<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    state:    &mut ChromeState,
    view:     &ChromeView<'_>,
    settings: &ChromeSettings,
    kind:     &ChromeRenderKind,
    layer:    &LayerId,
) -> Option<WidgetId> {
    let rect = layout.rect_for_chrome()?;
    Some(register_context_manager_chrome(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, layer,
    ))
}

// ---------------------------------------------------------------------------
// Tab width (duplicated locally to avoid coupling to render internals)
// ---------------------------------------------------------------------------

fn tab_w(label: &str, state: &ChromeState, i: usize, padding_h: f64, close_size: f64) -> f64 {
    if let Some(&w) = state.tab_widths.get(i) {
        return w;
    }
    let text_w = label.len() as f64 * 7.0;
    padding_h + text_w + close_size + padding_h
}

// ---------------------------------------------------------------------------
// chrome_hit_test
// ---------------------------------------------------------------------------

/// Convert a pointer position into a `ChromeHit`.
///
/// - `rect`   — bounding rect of the entire chrome strip (full window width, 32 px).
/// - `point`  — pointer position in window-relative logical pixels.
///
/// Hit-test order mirrors `chrome-deep.md` §3:
/// corners → edges → chrome zone (y < chrome_height) → None.
pub fn chrome_hit_test(
    state:    &ChromeState,
    view:     &ChromeView<'_>,
    settings: &ChromeSettings,
    kind:     &ChromeRenderKind,
    rect:     Rect,
    point:    (f64, f64),
) -> ChromeHit {
    let (px, py) = point;
    let style = settings.style.as_ref();
    let h     = style.chrome_height();
    let bw    = style.border_zone();
    let w     = rect.width;

    // Full window rect (assumes chrome is at top of a resizable window)
    // For resize detection we test against the chrome rect edges.
    let lx = rect.x;
    let rx = rect.x + w;
    let ty = rect.y;
    let by = rect.y + h;

    // --- Corners (take precedence over edges) ---
    if px < lx + bw && py < ty + bw {
        return ChromeHit::ResizeCorner(ResizeCorner::TopLeft);
    }
    if px > rx - bw && py < ty + bw {
        return ChromeHit::ResizeCorner(ResizeCorner::TopRight);
    }
    if px < lx + bw && py > by - bw {
        return ChromeHit::ResizeCorner(ResizeCorner::BottomLeft);
    }
    if px > rx - bw && py > by - bw {
        return ChromeHit::ResizeCorner(ResizeCorner::BottomRight);
    }

    // --- Edges ---
    if py < ty + bw  { return ChromeHit::ResizeTop;    }
    if py > by - bw  { return ChromeHit::ResizeBottom; }
    if px < lx + bw  { return ChromeHit::ResizeLeft;   }
    if px > rx - bw  { return ChromeHit::ResizeRight;  }

    // Outside chrome strip vertically
    if py < rect.y || py > rect.y + h {
        return ChromeHit::None;
    }

    // Not a Custom kind — proceed with slot hit-testing
    if matches!(kind, ChromeRenderKind::Custom(_)) {
        return ChromeHit::None;
    }

    let bp_close_x     = rect.x + w - 46.0;
    let bp_maximize_x  = rect.x + w - 92.0;
    let bp_minimize_x  = rect.x + w - 138.0;
    let bp_cw_left     = bp_minimize_x - 36.0;
    let bp_menu_left   = bp_cw_left - 36.0;
    let bp_nw_left     = bp_menu_left - 36.0;

    let show_tabs     = !matches!(kind, ChromeRenderKind::WindowControlsOnly);
    let show_controls = !matches!(kind, ChromeRenderKind::Minimal);

    // --- Window control buttons (right-to-left) ---
    if show_controls {
        if px >= rect.x + bp_close_x - rect.x {
            return ChromeHit::CloseBtn;
        }
        if px >= bp_maximize_x {
            return ChromeHit::MaxBtn;
        }
        if px >= bp_minimize_x {
            return ChromeHit::MinBtn;
        }
        if show_tabs {
            if px >= bp_cw_left {
                return ChromeHit::CloseWindowBtn;
            }
            if view.show_menu_btn && px >= bp_menu_left {
                return ChromeHit::Menu;
            }
            if px >= bp_nw_left {
                return ChromeHit::NewWindowBtn;
            }
        }
    }

    // --- Tabs ---
    if show_tabs {
        let padding_h  = style.tab_padding_h();
        let close_size = style.tab_close_size();
        let tab_gap    = style.tab_gap();

        let mut x = rect.x + 4.0; // TAB_LEFT_MARGIN
        for (i, tab) in view.tabs.iter().enumerate() {
            let tw = tab_w(tab.label, state, i, padding_h, close_size);
            if px >= x && px < x + tw {
                // Close-X sub-zone
                if tab.closable {
                    let close_bw = close_size;
                    let cx = x + tw - close_bw;
                    if px >= cx {
                        return ChromeHit::CloseTab(i);
                    }
                }
                return ChromeHit::Tab(i);
            }
            x += tw + tab_gap;
        }

        // New-tab "+" button
        if view.show_new_tab_btn && px >= x && px < x + 28.0 {
            return ChromeHit::NewTab;
        }
        let x_after_new_tab = x + 28.0;

        // Caption drag zone
        let drag_end = bp_nw_left;
        if px >= x_after_new_tab && px < rect.x + drag_end {
            return ChromeHit::Drag;
        }
    } else {
        // WindowControlsOnly: all non-button area is drag
        if px < bp_minimize_x {
            return ChromeHit::Drag;
        }
    }

    ChromeHit::None
}

// ---------------------------------------------------------------------------
// handle_chrome_action
// ---------------------------------------------------------------------------

/// Map a `ChromeHit` (produced by `chrome_hit_test`) to a `ChromeAction`.
///
/// Call this on pointer-up / click events to translate the hit zone into a
/// semantic action that the caller can dispatch.
pub fn handle_chrome_action(hit: ChromeHit) -> ChromeAction {
    match hit {
        ChromeHit::Tab(i)         => ChromeAction::SelectTab(i),
        ChromeHit::CloseTab(i)    => ChromeAction::CloseTab(i),
        ChromeHit::NewTab         => ChromeAction::NewTab,
        ChromeHit::NewWindowBtn   => ChromeAction::NewWindow,
        ChromeHit::Menu           => ChromeAction::OpenMenu,
        ChromeHit::Drag           => ChromeAction::WindowDragStart,
        ChromeHit::MinBtn         => ChromeAction::Minimize,
        ChromeHit::MaxBtn         => ChromeAction::MaximizeRestore,
        ChromeHit::CloseBtn       => ChromeAction::CloseApp,
        ChromeHit::CloseWindowBtn => ChromeAction::CloseWindow,
        ChromeHit::ResizeCorner(_)
        | ChromeHit::ResizeTop
        | ChromeHit::ResizeBottom
        | ChromeHit::ResizeLeft
        | ChromeHit::ResizeRight  => ChromeAction::BeginResize(hit),
        ChromeHit::None           => ChromeAction::None,
    }
}
