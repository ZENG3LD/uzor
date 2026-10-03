//! Chrome input helpers.
//!
//! Re-exports `register_input_coordinator_chrome` from `render.rs` and adds:
//! - `chrome_hit_test` — converts a pointer position to a `ChromeHit`
//! - `handle_chrome_action` — maps a `ChromeHit` to a `ChromeAction`

pub use super::render::register_input_coordinator_chrome;


use super::settings::ChromeSettings;
use super::state::ChromeState;
use super::types::{ChromeAction, ChromeHit, ChromeRenderKind, ChromeView, ResizeCorner};
use crate::core::types::Rect;


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

    // Outside chrome strip vertically — bail before any resize-edge match,
    // otherwise body clicks fall into ResizeBottom because `py > by - bw`
    // is true for the entire window body below the chrome strip.
    if py < rect.y || py > rect.y + h {
        return ChromeHit::None;
    }

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

    // --- Edges (only inside the chrome strip) ---
    if py < ty + bw  { return ChromeHit::ResizeTop;    }
    if py > by - bw  { return ChromeHit::ResizeBottom; }
    if px < lx + bw  { return ChromeHit::ResizeLeft;   }
    if px > rx - bw  { return ChromeHit::ResizeRight;  }

    // Not a Custom kind — proceed with slot hit-testing
    if matches!(kind, ChromeRenderKind::Custom(_)) {
        return ChromeHit::None;
    }

    let bp_close_x    = rect.x + w - 46.0;
    // When show_maximize is false, minimize shifts to w-92 (one slot), close stays w-46.
    let bp_maximize_x = if view.show_maximize {
        Some(rect.x + w - 92.0)
    } else {
        None
    };
    let bp_minimize_x = if view.show_maximize {
        rect.x + w - 138.0
    } else {
        rect.x + w - 92.0
    };

    // Pair rule: enabling new_window without close_window leaves no way to
    // close the spawned window, so the chrome composite treats them as a
    // pair when the caller only flipped `show_new_window_btn`.
    let effective_close_window = view.show_close_window_btn || view.show_new_window_btn;

    // Compact left edges for the optional right-cluster group — mirrors
    // `ButtonPositions::compute` in render.rs.
    // When `view.menu_left` is true the menu button is on the LEFT side and
    // is NOT added to the right cluster cursor.
    let mut cursor = bp_minimize_x;
    let bp_cw_left        = if effective_close_window {
        cursor -= 36.0; Some(cursor)
    } else { None };
    let bp_menu_right_left = if view.show_menu_btn && !view.menu_left {
        cursor -= 36.0; Some(cursor)
    } else { None };
    let bp_nw_left        = if view.show_new_window_btn {
        cursor -= 36.0; Some(cursor)
    } else { None };
    // Left-side menu button position (window-absolute).
    let bp_menu_left_abs = if view.show_menu_btn && view.menu_left {
        Some(rect.x) // x = rect.x + 0
    } else { None };

    let show_tabs     = !matches!(kind, ChromeRenderKind::WindowControlsOnly);
    let show_controls = !matches!(kind, ChromeRenderKind::Minimal);

    // --- Window control buttons (right-to-left) ---
    if show_controls {
        if px >= bp_close_x    { return ChromeHit::CloseBtn; }
        if let Some(max_x) = bp_maximize_x {
            if px >= max_x { return ChromeHit::MaxBtn; }
        }
        if px >= bp_minimize_x { return ChromeHit::MinBtn;   }
        if show_tabs {
            if let Some(left) = bp_cw_left {
                if effective_close_window && px >= left && px < left + 36.0 {
                    return ChromeHit::CloseWindowBtn;
                }
            }
            if let Some(left) = bp_menu_right_left {
                if px >= left && px < left + 36.0 { return ChromeHit::Menu; }
            }
            if let Some(left) = bp_nw_left {
                if px >= left && px < left + 36.0 { return ChromeHit::NewWindowBtn; }
            }
        }
    }

    // --- Tabs ---
    if show_tabs {
        // Left-side menu button hit-test (before tabs).
        if let Some(lmx) = bp_menu_left_abs {
            if px >= lmx && px < lmx + 36.0 {
                return ChromeHit::Menu;
            }
        }

        let padding_h  = style.tab_padding_h();
        let close_size = style.tab_close_size();
        let tab_gap    = style.tab_gap();

        // Tab area starts after the left-side menu button (if present).
        // TAB_LEFT_MARGIN = 0.0 (mirrors render.rs const).
        let tab_start = if bp_menu_left_abs.is_some() {
            rect.x + 36.0 // MENU_BTN_WIDTH
        } else {
            rect.x // TAB_LEFT_MARGIN == 0.0
        };

        let mut x = tab_start;
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
        // Advance past the "+" only when it is actually shown — mirrors
        // render.rs, which advances `x` by NEW_TAB_BTN_WIDTH conditionally.
        // (Unconditional +28 left a dead 28 px strip right of a left menu.)
        let x_after_new_tab = if view.show_new_tab_btn { x + 28.0 } else { x };

        // Caption drag zone — extends up to the leftmost enabled optional
        // right-cluster button, or to Min if none are enabled.
        let drag_end = bp_nw_left.or(bp_menu_right_left).or(bp_cw_left).unwrap_or(bp_minimize_x);
        if px >= x_after_new_tab && px < drag_end {
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
